use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Write};
use std::path::PathBuf;

use clap::Parser;
use encoding_rs::*;
use csv::ReaderBuilder;

/// Analyse les valeurs distinctes d'un champ dans un CSV, avec comptage.
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

    /// Nom du champ à analyser (optionnel si index fourni)
    #[arg(long)]
    field_name: Option<String>,

    /// Index du champ à analyser (optionnel si nom fourni, commence à 0)
    #[arg(long)]
    field_index: Option<usize>,

    /// Nombre maximum de lignes à lire (optionnel)
    #[arg(short, long)]
    max: Option<usize>,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    if args.field_name.is_none() && args.field_index.is_none() {
        anyhow::bail!("Veuillez spécifier --field-name ou --field-index");
    }

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
        .has_headers(true)
        .from_reader(transcoded);

    let headers = csv_reader.headers()?.clone();

    let field_idx = if let Some(idx) = args.field_index {
        idx
    } else if let Some(name) = args.field_name {
        headers.iter().position(|h| h == name).ok_or_else(|| anyhow::anyhow!("Champ '{name}' non trouvé dans l'entête"))?
    } else {
        unreachable!()
    };

    println!("Analyse du champ index {field_idx} : '{}'", headers.get(field_idx).unwrap_or("??"));

    let mut count = 0usize;
    let mut distribution: HashMap<String, usize> = HashMap::new();

    for result in csv_reader.records() {
        let record = result?;
        let value = record.get(field_idx).unwrap_or("").trim().to_string();

        *distribution.entry(value).or_insert(0) += 1;

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

    println!("Valeurs distinctes pour le champ :");
    let mut entries: Vec<_> = distribution.into_iter().collect();
    entries.sort_by(|a, b| b.1.cmp(&a.1)); // tri décroissant par fréquence

    for (val, freq) in entries {
        println!("{freq} : '{val}'");
    }

    Ok(())
}
