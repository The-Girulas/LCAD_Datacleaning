use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

use clap::Parser;
use encoding_rs::*;

/// Analyse tolérante des valeurs d'un champ dans un CSV corrompu.
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

    /// Index du champ à analyser (commence à 0)
    #[arg(long)]
    field_index: usize,

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

    let reader = BufReader::new(transcoded);

    let delimiter = if args.delimiter == "\\t" {
        '\t'
    } else {
        args.delimiter.chars().next().unwrap()
    };

    let mut count = 0usize;
    let mut distribution: HashMap<String, usize> = HashMap::new();

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

        let value = fields.get(args.field_index).unwrap_or(&"".to_string()).clone();
        *distribution.entry(value).or_insert(0) += 1;

        count += 1;
        if count % 100_000 == 0 {
            println!("Lignes lues : {count}");
        }

        if let Some(max_lines) = args.max {
            if count >= max_lines {
                println!("Limite de {max_lines} lignes atteinte.");
                break;
            }
        }
    }

    println!("Valeurs distinctes pour le champ index {} :", args.field_index);
    let mut entries: Vec<_> = distribution.into_iter().collect();
    entries.sort_by(|a, b| b.1.cmp(&a.1)); // tri décroissant

    for (val, freq) in entries {
        println!("{freq} : '{val}'");
    }

    Ok(())
}
