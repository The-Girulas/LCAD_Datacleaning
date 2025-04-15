use std::fs::File;
use std::io::{BufReader, Write};
use std::path::PathBuf;

use clap::Parser;
use encoding_rs::*;
use csv::ReaderBuilder;

/// Compte le nombre de lignes d'un fichier CSV, avec options d'encodage, séparateur, limite, progression.
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

    /// Nombre maximum de lignes à lire (optionnel)
    #[arg(short, long)]
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

    let mut csv_reader = ReaderBuilder::new()
        .delimiter(delimiter_byte)
        .has_headers(false)
        .flexible(true)
        .from_reader(transcoded);

    let mut count = 0usize;

    for result in csv_reader.records() {
        let _ = result?; // ignore le contenu
        count += 1;

        if count % 100_000 == 0 {
            print!("\rLignes lues : {count}");
            std::io::stdout().flush().unwrap();
        }

        if let Some(max_lines) = args.max {
            if count >= max_lines {
                println!("Limite de {max_lines} lignes atteinte.");
                break;
            }
        }
    }

    println!("Nombre total de lignes lues : {count}");

    Ok(())
}
