use std::fs::File;
use std::io::{BufReader, Write};
use std::path::PathBuf;

use clap::Parser;
use encoding_rs::*;
use csv::ReaderBuilder;
use indicatif::{ProgressBar, ProgressStyle}; // Added indicatif imports

/// Extraction de l'entête d'un fichier CSV, en gérant encodage et séparateur personnalisés.
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
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let pb = ProgressBar::new(1); // Initialize ProgressBar, assuming 1 line for header
    pb.set_style(ProgressStyle::default_spinner()
        .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ ") // Spinner characters
        .template("{spinner:.green} [{elapsed_precise}] {pos} lines processed ({per_sec})")
        .unwrap_or_else(|_| ProgressStyle::default_spinner())); // Fallback style

    // Ouvre le fichier brut
    let file = File::open(&args.file).map_err(|e| {
        pb.finish_with_message(format!("Error: Could not open file {:?}: {}", args.file, e));
        e
    })?;
    let mut reader = BufReader::new(file);

    // Détecte l'encodage
    let encoding = match args.encoding.to_lowercase().as_str() {
        "utf-8" => UTF_8,
        "windows-1252" => WINDOWS_1252,
        "iso-8859-1" => WINDOWS_1252,
        other => {
            eprintln!("Encodage non supporté: {other}, utilisation de utf-8 par défaut");
            UTF_8
        }
    };

    // Décode en UTF-8 à la volée
    let transcoded = encoding_rs_io::DecodeReaderBytesBuilder::new()
        .encoding(Some(encoding))
        .build(reader);

    // Crée un lecteur CSV avec séparateur personnalisé
    let delimiter_byte = if args.delimiter == "\\t" {
        b'\t'
    } else {
        // Ensure delimiter is not empty and take the first byte.
        if args.delimiter.is_empty() {
            pb.finish_with_message("Error: Delimiter cannot be empty.");
            return Err(anyhow::anyhow!("Delimiter cannot be empty. Use '\\t' for tab."));
        }
        args.delimiter.as_bytes()[0]
    };

    let mut csv_reader = ReaderBuilder::new()
        .delimiter(delimiter_byte)
        .has_headers(false) // on veut lire la première ligne brute
        .from_reader(transcoded);

    // Lit la première ligne (l'entête)
    let header_record = csv_reader
        .records()
        .next()
        .ok_or_else(|| {
            pb.finish_with_message("Error: File empty or initial read error.");
            anyhow::anyhow!("Fichier vide ou erreur de lecture")
        })??;
    
    pb.inc(1); // Increment progress after successfully reading the header record

    let nb_vars = header_record.len();
    println!("Nombre de variables détectées dans l'entête : {nb_vars}");

    // Prépare les deux colonnes
    let mut original: Vec<(usize, &str)> = header_record.iter().enumerate().collect();
    let mut alpha: Vec<(usize, &str)> = header_record.iter().enumerate().collect();
    alpha.sort_by_key(|&(_, v)| v.to_ascii_lowercase());

    // Affichage joli en console
    println!("\n{:^6} | {:<30} || {:^6} | {:<30}", "Idx", "Ordre d'origine", "Idx α", "Ordre alphabétique");
    println!("{:-<6}-+-{:-<30}-++-{:-<6}-+-{:-<30}", "", "", "", "");
    for i in 0..original.len().max(alpha.len()) {
        let (idx_o, var_o) = original.get(i).copied().unwrap_or((0, ""));
        let (idx_a, var_a) = alpha.get(i).copied().unwrap_or((0, ""));
        println!("{:^6} | {:<30} || {:^6} | {:<30}", idx_o, var_o, idx_a, var_a);
    }

    // Sauvegarde dans ListeVariablesContrats.txt
    let mut out = File::create("ListeVariablesContrats.txt").map_err(|e| {
        pb.finish_with_message(format!("Error: Could not create output file: {}", e));
        e
    })?;
    writeln!(out, "{:^6} | {:<30} || {:^6} | {:<30}", "Idx", "Ordre d'origine", "Idx α", "Ordre alphabétique")?;
    writeln!(out, "{:-<6}-+-{:-<30}-++-{:-<6}-+-{:-<30}", "", "", "", "")?;
    for i in 0..original.len().max(alpha.len()) {
        let (idx_o, var_o) = original.get(i).copied().unwrap_or((0, ""));
        let (idx_a, var_a) = alpha.get(i).copied().unwrap_or((0, ""));
        writeln!(out, "{:^6} | {:<30} || {:^6} | {:<30}", idx_o, var_o, idx_a, var_a)?;
    }
    
    pb.finish_with_message("Header extracted."); // Finish progress bar
    println!("Entête extraite et sauvegardée dans ListeVariablesContrats.txt (double colonne)");

    Ok(())
}
