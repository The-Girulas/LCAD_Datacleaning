//! Hyper analyseur CSV : réalise en un seul passage l'extraction d'entête, le comptage de lignes, la distribution du nombre de champs, l'analyse de valeurs de champs, et la réparation automatique du CSV.
//! Usage : voir README

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::PathBuf;

use clap::Parser;
use encoding_rs::*;

/// Hyper analyseur CSV : tout en un, un seul passage sur le fichier.
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

    /// Index des champs à analyser (ex: 2,5,10)
    #[arg(long, value_delimiter = ',')]
    analyze_fields: Vec<usize>,

    /// Nombre de champs attendu (pour la réparation)
    #[arg(long)]
    expected_fields: usize,

    /// Fichier de sortie corrigé
    #[arg(long, default_value = "hyper_corrected.csv")]
    output: PathBuf,

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

    let out_file = File::create(&args.output)?;
    let mut writer = BufWriter::new(out_file);

    let mut line_count = 0usize;
    let mut field_count_dist: HashMap<usize, usize> = HashMap::new();
    let mut field_value_dist: Vec<HashMap<String, usize>> = vec![HashMap::new(); args.analyze_fields.len()];
    let mut header: Option<Vec<String>> = None;

    for (i, line_result) in reader.lines().enumerate() {
        let line = line_result?;
        let mut in_quotes = false;
        let mut fields = Vec::new();
        let mut current = Vec::new();

        let bytes = line.as_bytes();
        let mut idx = 0;
        while idx < bytes.len() {
            let c = bytes[idx];
            if c == b'"' {
                in_quotes = !in_quotes;
                current.push(c);
            } else if c == delimiter as u8 && !in_quotes {
                fields.push(String::from_utf8_lossy(&current).trim_matches('"').to_string());
                current.clear();
            } else {
                current.push(c);
            }
            idx += 1;
        }
        fields.push(String::from_utf8_lossy(&current).trim_matches('"').to_string());

        // Extraction de l'entête (première ligne)
        if i == 0 {
            header = Some(fields.clone());
            let entete = fields.join(&args.delimiter.to_string());
            let mut entete_file = File::create("ListeVariablesContrats.txt")?;
            writeln!(entete_file, "{entete}")?;
        }

        // Comptage du nombre de champs
        *field_count_dist.entry(fields.len()).or_insert(0) += 1;

        // Analyse des valeurs de champs
        for (j, &field_idx) in args.analyze_fields.iter().enumerate() {
            let value = fields.get(field_idx).unwrap_or(&"".to_string()).clone();
            *field_value_dist[j].entry(value).or_insert(0) += 1;
        }

        // Réparation auto (fusion des champs en trop)
        let line_to_write = if fields.len() == args.expected_fields {
            fields.join(&args.delimiter.to_string())
        } else if fields.len() > args.expected_fields {
            let mut fixed_fields = Vec::new();
            fixed_fields.extend(fields[..args.expected_fields - 1].iter().cloned());
            let merged: String = fields[args.expected_fields - 1..].join(&args.delimiter.to_string());
            fixed_fields.push(merged);
            fixed_fields.join(&args.delimiter.to_string())
        } else {
            let mut bad_fields = vec![format!("#BAD ({} champs)", fields.len())];
            bad_fields.extend(fields);
            bad_fields.join(&args.delimiter.to_string())
        };
        writeln!(writer, "{line_to_write}")?;

        line_count += 1;
        if line_count % 100_000 == 0 {
            print!("\rLignes traitées : {line_count}");
            std::io::stdout().flush().unwrap();
        }

        if let Some(max_lines) = args.max {
            if line_count >= max_lines {
                println!("Limite de {max_lines} lignes atteinte.");
                break;
            }
        }
    }

    writer.flush()?;

    println!("\nNombre total de lignes lues : {line_count}");
    println!("Distribution du nombre de champs par ligne :");
    let mut keys: Vec<_> = field_count_dist.keys().cloned().collect();
    keys.sort();
    for k in keys {
        let v = field_count_dist.get(&k).unwrap();
        println!("{k} champs : {v} lignes");
    }

    if let Some(header) = header {
        for (j, &field_idx) in args.analyze_fields.iter().enumerate() {
            let field_name = if let Some(name) = header.get(field_idx) {
                name.clone()
            } else {
                format!("Champ {field_idx}")
            };
            println!("\nValeurs distinctes pour le champ {field_idx} ('{field_name}') :");
            let mut entries: Vec<_> = field_value_dist[j].iter().collect();
            entries.sort_by(|a, b| b.1.cmp(a.1));
            for (val, freq) in entries.iter().take(20) {
                println!("{freq} : '{val}'");
            }
            if entries.len() > 20 {
                println!("... ({} valeurs distinctes au total)", entries.len());
            }
        }
    }

    println!("\nFichier corrigé écrit dans {:?}", args.output);

    Ok(())
}
