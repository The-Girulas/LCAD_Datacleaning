use std::fs::File;
use std::io::{BufReader, BufWriter, Write};
use std::path::PathBuf;

use clap::Parser;
use encoding_rs::*;

/// Corrige un CSV en filtrant ou marquant les lignes incohérentes (nombre de champs inattendu).
#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    /// Chemin du fichier CSV source
    #[arg(short, long)]
    file: PathBuf,

    /// Encodage du fichier (utf-8, windows-1252, iso-8859-1, etc.)
    #[arg(short = 'e', long, default_value = "utf-8")]
    encoding: String,

    /// Séparateur de champ (ex: ',' ou ';' ou '\\t')
    #[arg(short = 'd', long, default_value = ",")]
    delimiter: String,

    /// Nombre de champs attendu (ex: 24)
    #[arg(short = 'n', long)]
    expected_fields: usize,

    /// Fichier de sortie corrigé
    #[arg(short = 'o', long, default_value = "corrected.csv")]
    output: PathBuf,

    /// Nombre maximum de lignes à lire (optionnel)
    #[arg(short = 'm', long)]
    max: Option<usize>,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let file = File::open(&args.file)?;
    let reader = BufReader::new(file);

    let encoding = match args.encoding.to_lowercase().as_str() {
        "utf-8" => UTF_8,
        "windows-1252" => WINDOWS_1252,
        "iso-8859-1" => WINDOWS_1252,
        other => {
            eprintln!("Encodage non supporté: {other}, utilisation de utf-8 par défaut");
            UTF_8
        }
    };

    let transcoded = encoding_rs_io::DecodeReaderBytesBuilder::new()
        .encoding(Some(encoding))
        .build(reader);

    let delimiter_byte = if args.delimiter == "\\t" {
        b'\t'
    } else {
        args.delimiter.as_bytes()[0]
    };

    let out_file = File::create(&args.output)?;
    let mut writer = BufWriter::new(out_file);

    let mut count = 0usize;
    let mut ok = 0usize;
    let mut bad = 0usize;

    use std::io::BufRead;
    let reader = BufReader::new(transcoded);

    for line_result in reader.lines() {
        let line = line_result?;
        let mut in_quotes = false;
        let mut fields = Vec::new();
        let mut current = String::new();

        for c in line.chars() {
            if c == '"' {
                in_quotes = !in_quotes;
                current.push(c);
            } else if c == (delimiter_byte as char) && !in_quotes {
                fields.push(current.trim_matches('"').to_string());
                current.clear();
            } else {
                current.push(c);
            }
        }
        fields.push(current.trim_matches('"').to_string());

        count += 1;

        let line_to_write = if fields.len() == args.expected_fields {
            ok += 1;
            fields.join(&(args.delimiter.clone()))
        } else {
            bad += 1;
            let mut bad_line = vec![format!("#BAD ({} champs)", fields.len())];
            bad_line.extend(fields);
            bad_line.join(&(args.delimiter.clone()))
        };

        writeln!(writer, "{line_to_write}")?;

        if count % 100_000 == 0 {
            print!("\rLignes traitées : {count}");
            std::io::stdout().flush().unwrap();
        }

        if let Some(max_lines) = args.max {
            if count >= max_lines {
                println!("Limite de {max_lines} lignes atteinte.");
                break;
            }
        }
    }

    writer.flush()?;

    println!("Total lignes traitées : {count}");
    println!("Lignes correctes      : {ok}");
    println!("Lignes incorrectes    : {bad}");
    println!("Fichier corrigé écrit dans {:?}", args.output);

    Ok(())
}
