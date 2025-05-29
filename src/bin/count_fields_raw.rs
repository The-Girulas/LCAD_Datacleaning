use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

use clap::Parser;
use encoding_rs::*;

/// Analyse brute du nombre de champs par ligne dans un CSV, sans parser strict.
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

    /// Séparateur décimal ambigu (ex: ',' si virgule est aussi séparateur décimal)
    #[arg(long)]
    decimal: Option<String>,

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

    let decimal_sep = args.decimal.as_ref().and_then(|s| s.chars().next());

    /// Vérifie si le caractère à l'index `i` dans `line` est un séparateur décimal entouré de chiffres
    fn is_decimal_separator(line: &str, i: usize, _decimal_sep: char) -> bool {
        let chars: Vec<char> = line.chars().collect();
        if i == 0 || i + 1 >= chars.len() {
            return false;
        }
        // gauche
        let mut j = i;
        let mut found_digit_left = false;
        while j > 0 {
            j -= 1;
            let c = chars[j];
            if c.is_ascii_digit() {
                found_digit_left = true;
                continue;
            } else if c == ' ' || c == '+' || c == '-' || c == '.' || c == '\'' {
                continue;
            } else {
                break;
            }
        }
        // droite
        let mut k = i + 1;
        let mut found_digit_right = false;
        while k < chars.len() {
            let c = chars[k];
            if c.is_ascii_digit() {
                found_digit_right = true;
                k += 1;
                continue;
            } else if c == ' ' {
                k += 1;
                continue;
            } else {
                break;
            }
        }
        found_digit_left && found_digit_right
    }

    let mut count = 0usize;
    let mut distribution: HashMap<usize, usize> = HashMap::new();

    for line_result in reader.lines() {
        let line = line_result?;
        let mut in_quotes = false;
        let mut field_count = 1; // au moins un champ

        let bytes = line.as_bytes();
        let mut idx = 0;
        while idx < bytes.len() {
            let c = bytes[idx];
            if c == b'"' {
                in_quotes = !in_quotes;
            } else if c == delimiter as u8 && !in_quotes {
                // (optionnel) gestion du séparateur décimal ambigu
                if let Some(decimal_c) = decimal_sep {
                    if decimal_c as u8 == delimiter as u8 {
                        // On ne gère pas ici l'ambiguïté décimale pour l'optimisation, à réintégrer si besoin
                    }
                }
                field_count += 1;
            }
            idx += 1;
        }

        *distribution.entry(field_count).or_insert(0) += 1;

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

    println!("\nDistribution brute du nombre de champs par ligne :");
    let mut keys: Vec<_> = distribution.keys().cloned().collect();
    keys.sort();
    for k in keys {
        let v = distribution.get(&k).unwrap();
        println!("{k} champs : {v} lignes");
    }

    Ok(())
}
