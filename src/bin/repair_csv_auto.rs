use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::PathBuf;

use clap::Parser;
use encoding_rs::*;

/// Correction automatique d'un CSV corrompu : fusionne les champs éclatés, marque les lignes irrécupérables.
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

    /// Nombre de champs attendu
    #[arg(short = 'n', long)]
    expected_fields: usize,

    /// Fichier de sortie corrigé
    #[arg(short = 'o', long, default_value = "corrected_auto.csv")]
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

    let reader = BufReader::new(transcoded);

    let delimiter = if args.delimiter == "\\t" {
        '\t'
    } else {
        args.delimiter.chars().next().unwrap()
    };

    let out_file = File::create(&args.output)?;
    let mut writer = BufWriter::new(out_file);

    let mut count = 0usize;
    let mut ok = 0usize;
    let mut fixed = 0usize;
    let mut bad = 0usize;

    for line_result in reader.lines() {
        let line = line_result?;
        let mut in_quotes = false;
        let mut fields = Vec::new();
        let mut current = String::new();

        for c in line.chars() {
            if c == '"' {
                in_quotes = !in_quotes;
                current.push(c);
            } else if c == delimiter && !in_quotes {
                fields.push(current.trim_matches('"').to_string());
                current.clear();
            } else {
                current.push(c);
            }
        }
        fields.push(current.trim_matches('"').to_string());

        let line_to_write = if fields.len() == args.expected_fields {
            ok += 1;
            fields.join(&args.delimiter)
        } else if fields.len() > args.expected_fields {
            // Tenter de fusionner les champs en trop dans le dernier champ valide
            fixed += 1;
            let mut fixed_fields = Vec::new();
            fixed_fields.extend(fields[..args.expected_fields - 1].iter().cloned());
            let merged: String = fields[args.expected_fields - 1..].join(&args.delimiter);
            fixed_fields.push(merged);
            fixed_fields.join(&args.delimiter)
        } else {
            // Ligne irrécupérable : marquer
            bad += 1;
            let mut bad_fields = vec![format!("#BAD ({} champs)", fields.len())];
            bad_fields.extend(fields);
            bad_fields.join(&args.delimiter)
        };

        writeln!(writer, "{line_to_write}")?;

        count += 1;
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
    println!("Lignes corrigées      : {fixed}");
    println!("Lignes irrécupérables : {bad}");
    println!("Fichier corrigé écrit dans {:?}", args.output);

    Ok(())
}
