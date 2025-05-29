use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Write}; // Write is not strictly needed for stdout flushing
use std::path::PathBuf;
use std::time::Duration; // For steady tick

use clap::Parser;
use encoding_rs::*;
use indicatif::{ProgressBar, ProgressStyle}; // Added indicatif imports

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

    let pb = if let Some(max_val) = args.max {
        ProgressBar::new(max_val as u64)
    } else {
        ProgressBar::new_spinner()
    };

    let style = ProgressStyle::default_spinner()
        .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ ")
        .template("{spinner:.green} [{elapsed_precise}] {pos} records processed ({per_sec})")
        .unwrap_or_else(|_| ProgressStyle::default_spinner());
    pb.set_style(style);

    if args.max.is_none() {
        pb.enable_steady_tick(Duration::from_millis(100));
    }

    let file = File::open(&args.file).map_err(|e| {
        pb.finish_with_message(format!("Error: Could not open file {:?}: {}", args.file, e));
        e
    })?;
    let buf_file_reader = BufReader::new(file); // Renamed to avoid confusion

    let encoding = match args.encoding.to_lowercase().as_str() {
        "utf-8" => UTF_8,
        "windows-1252" => WINDOWS_1252,
        "iso-8859-1" => WINDOWS_1252,
        other => {
            eprintln!("Encodage non supporté: {other}, utilisation de utf-8 par défaut");
            UTF_8
        }
    };

    let transcoded_reader = encoding_rs_io::DecodeReaderBytesBuilder::new()
        .encoding(Some(encoding))
        .build(buf_file_reader);

    let line_reader = BufReader::new(transcoded_reader); // Renamed for clarity

    let delimiter_char = if args.delimiter == "\\t" {
        '\t'
    } else {
        // Ensure delimiter is not empty and take the first char.
        // This was already in the original code, but good to be mindful of with pb.
        args.delimiter.chars().next().ok_or_else(|| {
            pb.finish_with_message("Error: Delimiter cannot be empty.");
            anyhow::anyhow!("Delimiter cannot be empty. Use '\\t' for tab.")
        })?
    };

    let mut record_count = 0usize; // Renamed 'count' to 'record_count' for clarity with instructions
    let mut distribution: HashMap<String, usize> = HashMap::new();
    let mut limit_reached = false;

    for line_result in line_reader.lines() {
        let line = match line_result {
            Ok(ln) => ln,
            Err(e) => {
                pb.abandon_with_message(format!("Error reading line after {} records: {}", record_count, e));
                return Err(e.into());
            }
        };

        // Manual CSV parsing logic from the original code
        let mut in_quotes = false;
        let mut fields = Vec::new();
        let mut current_field_buffer = String::new(); // Renamed 'current'

        for c in line.chars() {
            if c == '"' {
                in_quotes = !in_quotes;
                current_field_buffer.push(c); // Keep quotes for now, trim later
            } else if c == delimiter_char && !in_quotes {
                fields.push(current_field_buffer.trim_matches('"').to_string());
                current_field_buffer.clear();
            } else {
                current_field_buffer.push(c);
            }
        }
        fields.push(current_field_buffer.trim_matches('"').to_string());

        let value = fields.get(args.field_index).unwrap_or(&"".to_string()).clone();
        *distribution.entry(value).or_insert(0) += 1;

        record_count += 1;
        pb.inc(1);

        // Removed old progress print
        // if record_count % 100_000 == 0 {
        //     print!("\rLignes lues : {record_count}");
        //     std::io::stdout().flush().unwrap();
        // }

        if let Some(max_lines) = args.max {
            if record_count >= max_lines {
                // Removed old: println!("Limite de {max_lines} lignes atteinte.");
                limit_reached = true;
                break;
            }
        }
    }

    if limit_reached {
        if let Some(max_val) = args.max {
             pb.finish_with_message(format!("Analyzed {} records (limit of {} reached).", record_count, max_val));
        } else { // Should not happen if limit_reached is true
             pb.finish_with_message(format!("Analyzed {} records (limit reached).", record_count));
        }
    } else {
        pb.finish_with_message(format!("Analyzed {} records.", record_count));
    }

    // The distribution printing remains as it's the core output
    println!("Valeurs distinctes pour le champ index {} :", args.field_index);
    let mut entries: Vec<_> = distribution.into_iter().collect();
    entries.sort_by(|a, b| b.1.cmp(&a.1)); // tri décroissant

    for (val, freq) in entries {
        println!("{freq} : '{val}'");
    }

    Ok(())
}
