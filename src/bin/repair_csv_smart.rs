//! Réparation intelligente d’un CSV : pour chaque ligne incorrecte, on fusionne les cellules jusqu’à retrouver le format des lignes correctes.

use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::PathBuf;

use clap::Parser;
use encoding_rs::*;

/// Réparation intelligente : fusionne les champs des lignes incorrectes jusqu’à retrouver le format attendu.
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
    #[arg(short = 'o', long, default_value = "corrected_smart.csv")]
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

    let mut first_correct: Option<Vec<String>> = None;
    let mut incorrect_lines: Vec<(String, Vec<String>)> = Vec::new();

    // Première passe : traite et écrit directement les lignes correctes, stocke les incorrectes
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

        if fields.len() == args.expected_fields {
            if first_correct.is_none() {
                first_correct = Some(fields.clone());
            }
            writeln!(writer, "{}", fields.join(&delimiter.to_string()))?;
        } else {
            incorrect_lines.push((line, fields));
        }
    }

    // Utilise la première ligne correcte comme modèle
    let model = if let Some(l) = first_correct {
        l
    } else {
        eprintln!("Aucune ligne correcte trouvée pour servir de modèle.");
        return Ok(());
    };

    // Répare les lignes incorrectes
    for (raw, mut fields) in incorrect_lines {
        let mut repaired = Vec::new();
        let mut i = 0;
        let mut j = 0;
        while i < args.expected_fields && j < fields.len() {
            if !fields[j].is_empty() || model[i].is_empty() {
                repaired.push(std::mem::take(&mut fields[j]));
                i += 1;
                j += 1;
            } else if j + 1 < fields.len() {
                // Fusionne avec la cellule précédente
                if let Some(prev) = repaired.pop() {
                    let merged = format!("{}{}{}", prev, delimiter, fields[j].clone());
                    repaired.push(merged);
                } else {
                    repaired.push(fields[j].clone());
                }
                j += 1;
            } else {
                // Impossible de réparer, on marque la ligne
                repaired.clear();
                break;
            }
        }
        if repaired.len() == args.expected_fields {
            writeln!(writer, "{}", repaired.join(&delimiter.to_string()))?;
        } else {
            writeln!(writer, "#BAD ({} champs) : {}", fields.len(), raw)?;
        }
    }

    writer.flush()?;
    println!("Réparation intelligente terminée. Fichier corrigé : {:?}", args.output);

    Ok(())
}
