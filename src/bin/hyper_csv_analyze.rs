//! Hyper analyseur CSV : réalise en un seul passage l'extraction d'entête, le comptage de lignes, la distribution du nombre de champs, l'analyse de valeurs de champs, et la réparation automatique du CSV.
//! Usage : voir README

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, BufWriter, Write};
use std::path::PathBuf;
use std::time::Duration; // For steady tick

use clap::Parser;
use encoding_rs::*;
use csv::{ReaderBuilder, StringRecord};
use indicatif::{ProgressBar, ProgressStyle}; // Added indicatif imports

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

/// Extracts the header from the fields and writes it to "ListeVariablesContrats.txt".
fn extract_and_write_header(fields: &Vec<String>, delimiter_str: &str) -> std::io::Result<()> {
    let entete = fields.join(delimiter_str);
    let mut entete_file = File::create("ListeVariablesContrats.txt")?;
    writeln!(entete_file, "{entete}")?;
    Ok(())
}

/// Updates the distribution of field counts.
fn update_field_count_distribution(fields: &Vec<String>, field_count_dist: &mut HashMap<usize, usize>) {
    *field_count_dist.entry(fields.len()).or_insert(0) += 1;
}

/// Updates the distribution of values for specified fields.
fn update_field_value_distribution(
    fields: &Vec<String>,
    analyze_field_indices: &Vec<usize>,
    field_value_dist: &mut Vec<HashMap<String, usize>>,
) {
    for (j, &field_idx) in analyze_field_indices.iter().enumerate() {
        let value = fields.get(field_idx).unwrap_or(&"".to_string()).clone();
        if j < field_value_dist.len() { 
            *field_value_dist[j].entry(value).or_insert(0) += 1;
        }
    }
}

/// Repairs the line based on expected field count and writes it to the output writer.
fn repair_and_write_line(
    fields: &Vec<String>,
    expected_fields: usize,
    delimiter_str: &str,
    writer: &mut BufWriter<File>,
) -> std::io::Result<()> {
    let line_to_write = if fields.len() == expected_fields {
        fields.join(delimiter_str)
    } else if fields.len() > expected_fields && expected_fields > 0 { 
        let mut fixed_fields = Vec::new();
        fixed_fields.extend(fields.get(..expected_fields - 1).unwrap_or_default().iter().cloned());
        let merged: String = fields.get(expected_fields - 1..).unwrap_or_default().join(delimiter_str);
        fixed_fields.push(merged);
        fixed_fields.join(delimiter_str)
    } else { 
        let mut bad_fields = vec![format!("#BAD ({} champs)", fields.len())];
        bad_fields.extend(fields.iter().cloned());
        bad_fields.join(delimiter_str)
    };
    writeln!(writer, "{line_to_write}")?;
    Ok(())
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

    let input_file = File::open(&args.file).map_err(|e| {
        pb.finish_with_message(format!("Error: Could not open input file {:?}: {}", args.file, e));
        e
    })?;
    let raw_reader = BufReader::new(input_file);

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
        .build(raw_reader);

    let buf_transcoded_reader = BufReader::new(transcoded_reader);

    let delimiter_byte = if args.delimiter == "\\t" {
        b'\t'
    } else {
        args.delimiter.as_bytes().get(0).cloned().ok_or_else(|| {
            pb.finish_with_message("Error: Delimiter cannot be empty.");
            anyhow::anyhow!("Delimiter cannot be empty. Use '\\t' for tab.")
        })?
    };
    let delimiter_str = args.delimiter.replace("\\t", "\t");

    let mut csv_reader = ReaderBuilder::new()
        .delimiter(delimiter_byte)
        .has_headers(false)
        .flexible(true)
        .from_reader(buf_transcoded_reader);

    let out_file = File::create(&args.output).map_err(|e| {
        pb.finish_with_message(format!("Error: Could not create output file {:?}: {}", args.output, e));
        e
    })?;
    let mut writer = BufWriter::new(out_file);

    let mut line_count = 0usize;
    let mut field_count_dist: HashMap<usize, usize> = HashMap::new();
    let mut field_value_dist: Vec<HashMap<String, usize>> = vec![HashMap::new(); args.analyze_fields.len()];
    let mut header_fields: Option<Vec<String>> = None;
    let mut limit_reached = false;

    for (i, result) in csv_reader.records().enumerate() {
        let record: StringRecord = match result {
            Ok(rec) => rec,
            Err(e) => {
                pb.abandon_with_message(format!("Error reading CSV record after {} records: {}", line_count, e));
                return Err(e.into());
            }
        };
        let fields: Vec<String> = record.iter().map(|field| field.to_string()).collect();
        
        if i == 0 {
            if let Err(e) = extract_and_write_header(&fields, &delimiter_str) {
                pb.abandon_with_message(format!("Error extracting header: {}", e));
                return Err(e.into());
            }
            header_fields = Some(fields.clone());
        }

        line_count += 1;

        update_field_count_distribution(&fields, &mut field_count_dist);

        if !args.analyze_fields.is_empty() { 
            update_field_value_distribution(&fields, &args.analyze_fields, &mut field_value_dist);
        }

        if let Err(e) = repair_and_write_line(&fields, args.expected_fields, &delimiter_str, &mut writer) {
            pb.abandon_with_message(format!("Error writing repaired line after {} records: {}", line_count, e));
            return Err(e.into());
        }
        
        pb.inc(1);

        // Removed old progress print:
        // if line_count % 100_000 == 0 {
        //     print!("\rLignes traitées : {line_count}");
        //     std::io::stdout().flush().unwrap();
        // }

        if let Some(max_lines) = args.max {
            if line_count >= max_lines {
                // Removed old: println!("\nLimite de {max_lines} lignes atteinte.");
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
        format!("Analyzed and processed {} records (limit of {} reached). Corrected file written to {:?}", 
                line_count, args.max.unwrap_or(line_count), args.output)
    } else {
        format!("Analyzed and processed {} records. Corrected file written to {:?}", 
                line_count, args.output)
    };
    pb.finish_with_message(final_message);

    // Post-loop result printing (remains unchanged)
    println!("\nNombre total de lignes lues : {line_count}");
    println!("Distribution du nombre de champs par ligne :");
    let mut distribution_keys: Vec<_> = field_count_dist.keys().cloned().collect();
    distribution_keys.sort();
    for k in distribution_keys {
        let v = field_count_dist.get(&k).unwrap();
        println!("{k} champs : {v} lignes");
    }

    if let Some(ref actual_header_fields) = header_fields { 
        if !args.analyze_fields.is_empty() && !field_value_dist.is_empty() {
            for (j, &field_idx) in args.analyze_fields.iter().enumerate() {
                let field_name = actual_header_fields
                    .get(field_idx)
                    .map(String::as_str)
                    .unwrap_or_else(|| "Champ Inconnu"); 

                println!("\nValeurs distinctes pour le champ {field_idx} ('{field_name}') :");
                
                if j < field_value_dist.len() {
                    let mut entries: Vec<_> = field_value_dist[j].iter().collect();
                    entries.sort_by(|a, b| b.1.cmp(a.1)); 
                    for (val, freq) in entries.iter().take(20) {
                        println!("{freq} : '{val}'");
                    }
                    if entries.len() > 20 {
                        println!("... ({} valeurs distinctes au total)", entries.len());
                    }
                } else {
                     println!("Aucune donnée d'analyse pour l'index de champ {field_idx} (j={j})");
                }
            }
        }
    } else if !args.analyze_fields.is_empty() {
         println!("\nAnalyse de champs demandée, mais aucun entête n'a été extrait (fichier vide ou erreur de lecture de la première ligne).");
    }

    // The "Fichier corrigé écrit dans {:?}" is part of pb.finish_with_message,
    // so the original println! below is now redundant and has been removed.
    // println!("\nFichier corrigé écrit dans {:?}", args.output);

    Ok(())
}
