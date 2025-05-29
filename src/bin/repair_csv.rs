use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write}; // Write is not strictly needed for stdout flushing
use std::path::PathBuf;
use std::time::Duration; // For steady tick

use clap::Parser;
use encoding_rs::*;
use indicatif::{ProgressBar, ProgressStyle}; // Added indicatif imports

/// Corrige un CSV en filtrant ou marquant les lignes incohérentes (nombre de champs inattendu).
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
    #[arg(short = 'o', long, default_value = "corrected.csv")]
    output: PathBuf,

    /// Nombre maximum de lignes à lire (optionnel)
    #[arg(short = 'm', long)]
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
        .template("{spinner:.green} [{elapsed_precise}] {pos} lines processed ({per_sec})")
        .unwrap_or_else(|_| ProgressStyle::default_spinner());
    pb.set_style(style);

    if args.max.is_none() {
        pb.enable_steady_tick(Duration::from_millis(100));
    }

    let input_file = File::open(&args.file).map_err(|e| {
        pb.finish_with_message(format!("Error: Could not open input file {:?}: {}", args.file, e));
        e
    })?;
    let input_buf_reader = BufReader::new(input_file);

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
        .build(input_buf_reader);

    let line_reader = BufReader::new(transcoded_reader);

    // Delimiter for parsing (char) and for joining (str)
    let delimiter_char = if args.delimiter == "\\t" {
        '\t'
    } else {
        args.delimiter.chars().next().ok_or_else(|| {
            pb.finish_with_message("Error: Delimiter cannot be empty.");
            anyhow::anyhow!("Delimiter cannot be empty. Use '\\t' for tab.")
        })?
    };
    // The original code uses args.delimiter.clone() for joining, which is fine.
    // No need to create a separate delimiter_str unless we want to parse "\\t" for joining too.
    // The original code did not, it passed args.delimiter directly to join.

    let out_file = File::create(&args.output).map_err(|e| {
        pb.finish_with_message(format!("Error: Could not create output file {:?}: {}", args.output, e));
        e
    })?;
    let mut writer = BufWriter::new(out_file);

    let mut line_count = 0usize; // Renamed 'count' to 'line_count' as per plan
    let mut ok_lines = 0usize;    // Renamed 'ok'
    let mut bad_lines = 0usize;   // Renamed 'bad'
    let mut limit_reached = false;

    for line_result in line_reader.lines() {
        let line = match line_result {
            Ok(ln) => ln,
            Err(e) => {
                pb.abandon_with_message(format!("Error reading line after {} lines: {}", line_count, e));
                return Err(e.into());
            }
        };

        // Manual CSV parsing logic from the original code
        let mut in_quotes = false;
        let mut fields = Vec::new();
        let mut current_field_buffer = String::new();

        for c in line.chars() {
            if c == '"' {
                in_quotes = !in_quotes;
                current_field_buffer.push(c);
            } else if c == delimiter_char && !in_quotes {
                fields.push(current_field_buffer.trim_matches('"').to_string());
                current_field_buffer.clear();
            } else {
                current_field_buffer.push(c);
            }
        }
        fields.push(current_field_buffer.trim_matches('"').to_string());

        line_count += 1;

        let line_to_write = if fields.len() == args.expected_fields {
            ok_lines += 1;
            fields.join(&args.delimiter) // Original used args.delimiter.clone()
        } else {
            bad_lines += 1;
            let mut bad_line_parts = vec![format!("#BAD ({} champs)", fields.len())];
            bad_line_parts.extend(fields);
            bad_line_parts.join(&args.delimiter) // Original used args.delimiter.clone()
        };

        if let Err(e) = writeln!(writer, "{line_to_write}") {
            pb.abandon_with_message(format!("Error writing to output file after {} lines: {}", line_count, e));
            return Err(e.into());
        }
        
        pb.inc(1);

        // Removed old progress print
        // if line_count % 100_000 == 0 {
        //     print!("\rLignes traitées : {line_count}");
        //     std::io::stdout().flush().unwrap();
        // }

        if let Some(max_lines) = args.max {
            if line_count >= max_lines {
                // Removed old: println!("Limite de {max_lines} lignes atteinte.");
                limit_reached = true;
                break;
            }
        }
    }

    if let Err(e) = writer.flush() {
        pb.abandon_with_message(format!("Error flushing output file: {}", e));
        return Err(e.into());
    }

    let final_message = if limit_reached {
        format!("Processed {} lines (limit of {} reached). Repaired file written to {:?}", 
                line_count, args.max.unwrap_or(line_count), args.output)
    } else {
        format!("Processed {} lines. Repaired file written to {:?}", 
                line_count, args.output)
    };
    pb.finish_with_message(final_message);

    // These summary prints remain as they are post-processing info
    println!("Total lignes traitées : {line_count}");
    println!("Lignes correctes      : {ok_lines}");
    println!("Lignes incorrectes    : {bad_lines}");
    // The "Fichier corrigé écrit dans" is part of pb.finish_with_message now.
    // For consistency, we might want to remove the last original println or make pb message shorter.
    // Let's keep the original summary prints fully for now, and the pb message as defined in the task.

    Ok(())
}
