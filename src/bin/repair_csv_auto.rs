use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter};
use std::path::PathBuf;

use clap::Parser;
use encoding_rs::*;
use csv::WriterBuilder;

/// Correction automatique d'un CSV corrompu : fusionne les champs éclatés, marque les lignes irrécupérables.
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

    /// Nombre de champs attendu
    #[arg(short, long)]
    expected_fields: usize,

    /// Fichier de sortie corrigé
    #[arg(short, long, default_value = "corrected_auto.csv")]
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
    let mut writer = WriterBuilder::new()
        .delimiter(delimiter as u8)
        .from_writer(BufWriter::new(out_file));

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

        if fields.len() == args.expected_fields {
            writer.write_record(&fields)?;
            ok += 1;
        } else if fields.len() > args.expected_fields {
            // Tenter de fusionner les champs en trop dans le dernier champ valide
            let mut fixed_fields = Vec::new();
            fixed_fields.extend(fields[..args.expected_fields - 1].iter().cloned());
            let merged: String = fields[args.expected_fields - 1..].join(&args.delimiter);
            fixed_fields.push(merged);
            writer.write_record(&fixed_fields)?;
            fixed += 1;
        } else {
            // Ligne irrécupérable : marquer
            let mut bad_fields = vec![format!("#BAD ({} champs)", fields.len())];
            bad_fields.extend(fields);
            writer.write_record(&bad_fields)?;
            bad += 1;
        }

        count += 1;
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
    println!("Lignes corrigées      : {fixed}");
    println!("Lignes irrécupérables : {bad}");
    println!("Fichier corrigé écrit dans {:?}", args.output);

    Ok(())
}
