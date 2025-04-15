use std::fs::File;
use std::io::{BufReader, BufWriter, Write};
use std::path::PathBuf;

use clap::Parser;
use encoding_rs::*;
use csv::{ReaderBuilder, WriterBuilder};

/// Corrige un CSV en filtrant ou marquant les lignes incohérentes (nombre de champs inattendu).
#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    /// Chemin du fichier CSV source
    #[arg(short, long)]
    file: PathBuf,

    /// Encodage du fichier (utf-8, windows-1252, iso-8859-1, etc.)
    #[arg(short, long, default_value = "utf-8")]
    encoding: String,

    /// Séparateur de champ (ex: ',' ou ';' ou '\\t')
    #[arg(short, long, default_value = ",")]
    delimiter: String,

    /// Nombre de champs attendu (ex: 24)
    #[arg(short, long)]
    expected_fields: usize,

    /// Fichier de sortie corrigé
    #[arg(short, long, default_value = "corrected.csv")]
    output: PathBuf,

    /// Nombre maximum de lignes à lire (optionnel)
    #[arg(short, long)]
    max: Option<usize>,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let file = File::open(&args.file)?;
    let mut reader = BufReader::new(file);

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

    let mut csv_reader = ReaderBuilder::new()
        .delimiter(delimiter_byte)
        .has_headers(false)
        .from_reader(transcoded);

    let out_file = File::create(&args.output)?;
    let mut writer = WriterBuilder::new()
        .delimiter(delimiter_byte)
        .from_writer(BufWriter::new(out_file));

    let mut count = 0usize;
    let mut ok = 0usize;
    let mut bad = 0usize;

    for result in csv_reader.records() {
        let record = result?;
        count += 1;

        if record.len() == args.expected_fields {
            writer.write_record(&record)?;
            ok += 1;
        } else {
            // Option simple : ignorer ou marquer la ligne
            // Ici, on écrit une ligne commentée commençant par "#BAD"
            let mut line = vec![format!("#BAD ({} champs)", record.len())];
            line.extend(record.iter().map(|s| s.to_string()));
            writer.write_record(&line)?;
            bad += 1;
        }

        if count % 100_000 == 0 {
            println!("Lignes traitées : {count}");
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
