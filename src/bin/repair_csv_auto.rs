use std::fs::File;
use std::io::{BufReader, BufWriter, Write}; // Removed BufRead
use std::path::PathBuf;

use clap::Parser;
use csv; // Added csv crate
use encoding_rs; // Removed glob import, kept crate import for encoding_rs_io and explicit paths
use indicatif::{ProgressBar, ProgressStyle}; // Added indicatif

#[derive(Debug, Clone, PartialEq)]
enum ColumnType {
    Numeric, // Represents numbers (integers or floats)
    Text,    // Represents any other text
    Empty,   // Represents a column that was empty in all sample lines
}

/// Correction automatique d'un CSV corrompu : fusionne les champs éclatés, marque les lignes irrécupérables.
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

    /// Nombre de champs attendu
    #[arg(short = 'n', long)]
    expected_fields: usize,

    /// Fichier de sortie corrigé
    #[arg(short = 'o', long, default_value = "corrected_auto.csv")]
    output: PathBuf,

    /// Nombre maximum de lignes à lire (optionnel)
    #[arg(short = 'm', long)]
    max: Option<usize>,

    /// Séparateur décimal pour l'inférence de type numérique (ex: '.' ou ',')
    #[arg(long, default_value = ".")]
    decimal_separator: String,

    /// Nombre de lignes "correctes" à utiliser pour l'inférence de type (0 pour désactiver l'inférence)
    #[arg(long, default_value_t = 1000)]
    inference_lines: usize,
}

// Actual implementation for type inference function
fn infer_column_types(
    file_path: &PathBuf,
    encoding_str: &str,
    delimiter_byte: u8,
    expected_fields: usize,
    max_inference_lines: usize,
    decimal_separator: &str,
) -> anyhow::Result<Vec<ColumnType>> {
    if max_inference_lines == 0 {
        return Ok(Vec::new()); // No lines to infer from
    }
    if expected_fields == 0 {
        return Ok(Vec::new()); // No fields to infer types for
    }

    // Helper function for numeric parsing
    fn is_numeric(value: &str, decimal_sep: &str) -> bool {
        if value.is_empty() {
            return true; // Empty fields don't invalidate Numeric type for a column
        }
        let parsable_value = if decimal_sep != "." {
            value.replace(decimal_sep, ".")
        } else {
            value.to_string() // Avoid allocation if no replacement needed
        };
        parsable_value.parse::<f64>().is_ok()
    }

    let mut inferred_types: Vec<ColumnType> = vec![ColumnType::Empty; expected_fields];
    let mut good_lines_processed = 0;

    let file = File::open(file_path)?;
    let initial_reader = BufReader::new(file);

    let encoding_val = match encoding_str.to_lowercase().as_str() { // Renamed 'encoding' to 'encoding_val'
        "utf-8" => encoding_rs::UTF_8,
        "windows-1252" | "iso-8859-1" => encoding_rs::WINDOWS_1252, // Corrected mapping for iso-8859-1
        other => {
            // This case should ideally be handled before calling, or return an error
            eprintln!("(inférence) Encodage non supporté: {other}, utilisation de utf-8 par défaut");
            encoding_rs::UTF_8
        }
    };

    let transcoded_reader = encoding_rs_io::DecodeReaderBytesBuilder::new()
        .encoding(Some(encoding_val)) // Use renamed variable
        .build(initial_reader);

    let mut csv_reader = csv::ReaderBuilder::new()
        .delimiter(delimiter_byte)
        .has_headers(false)
        .from_reader(BufReader::new(transcoded_reader));

    for (line_num, record_result) in csv_reader.records().enumerate() {
        let record = match record_result {
            Ok(r) => r,
            Err(_err) => { // Renamed 'err' to '_err' as it's no longer used
                // Verbose error message removed as per requirement.
                // The line is skipped, and inference continues.
                // eprintln!(
                //     "Avertissement: Erreur de lecture CSV durant l'inférence à la ligne {}: {}. Ligne ignorée.",
                //     line_num + 1, 
                //     _err // Use renamed variable if eprinting
                // );
                continue; // Skip this problematic line
            }
        };

        if record.len() == expected_fields {
            good_lines_processed += 1;

            for i in 0..expected_fields {
                let field_value = record.get(i).unwrap_or("").trim();

                if field_value.is_empty() {
                    // Empty field; doesn't change current inferred type unless it's the first data
                    // If it's Empty, it remains Empty. If Numeric, remains Numeric. If Text, remains Text.
                    continue;
                }

                match inferred_types[i] {
                    ColumnType::Empty => {
                        if is_numeric(field_value, decimal_separator) {
                            inferred_types[i] = ColumnType::Numeric;
                        } else {
                            inferred_types[i] = ColumnType::Text;
                        }
                    }
                    ColumnType::Numeric => {
                        if !is_numeric(field_value, decimal_separator) {
                            inferred_types[i] = ColumnType::Text;
                        }
                    }
                    ColumnType::Text => {
                        // Already Text, stays Text
                    }
                }
            }

            if good_lines_processed % 200 == 0 && good_lines_processed > 0 { // Print progress occasionally
                print!("\rLignes correctes analysées pour l'inférence : {}/{}", good_lines_processed, max_inference_lines);
                std::io::stdout().flush()?;
            }


            if good_lines_processed >= max_inference_lines {
                break; // Reached desired number of lines for inference
            }
        }
    }
    if good_lines_processed > 0 { // Clear progress line
        println!();
    }


    // Finalize: change remaining Empty to Text
    for col_type in inferred_types.iter_mut() {
        if *col_type == ColumnType::Empty {
            *col_type = ColumnType::Text;
        }
    }

    if good_lines_processed == 0 && max_inference_lines > 0 {
         eprintln!("Avertissement: Aucune ligne avec le nombre de champs attendu ({}) n'a été trouvée pour l'inférence.", expected_fields);
         // All types will be Text due to the finalization loop, which is a safe default.
    }


    Ok(inferred_types)
}

// Helper for try_merge_fields: Checks if a value is compatible with a ColumnType.
fn is_field_type_compatible(
    value: &str,
    expected_type: &ColumnType,
    decimal_separator: &str,
) -> bool {
    match expected_type {
        ColumnType::Text => true,
        ColumnType::Empty => true, // Empty fields are compatible with columns initially inferred as Empty
        ColumnType::Numeric => {
            if value.is_empty() {
                return true; // Empty string is compatible with Numeric columns
            }
            let parsable_value = if decimal_separator != "." {
                value.replace(decimal_separator, ".")
            } else {
                // Avoid allocation if no replacement needed.
                // However, to_string() is used here because parsable_value needs to be owned for parse(),
                // and value is a &str. If value was already String, this could be optimized.
                // For this specific context, value is usually a slice of a String from CSV parsing or a merged String.
                value.to_string()
            };
            parsable_value.parse::<f64>().is_ok()
        }
    }
}

// Recursive function to try and merge fields based on inferred column types.
fn try_merge_fields<'a>(
    original_fields: &'a [String],
    current_field_index: usize, // Current index in original_fields
    target_col_index: usize,    // Current index in expected_types
    expected_types: &[ColumnType],
    decimal_separator: &str,
    delimiter_str: &str, // Original delimiter string for joining
    fixed_line_so_far: &mut Vec<String>,
) -> bool {
    // Base Case 1: All target columns have been successfully filled.
    if target_col_index == expected_types.len() {
        // If all original fields have also been consumed, it's a perfect match.
        return current_field_index == original_fields.len();
    }

    // Base Case 2: Ran out of original fields to process, but still have target columns to fill.
    if current_field_index == original_fields.len() {
        return false;
    }

    // Recursive Step: Try to merge 1 or more original fields to satisfy the current target_col_index.
    // The maximum number of fields we can merge is such that we leave enough fields for the remaining target columns.
    // (original_fields.len() - current_field_index) is num_fields_remaining_in_original.
    // (expected_types.len() - target_col_index) is num_target_cols_remaining.
    // So, we can try merging up to (num_fields_remaining_in_original - (num_target_cols_remaining - 1)) fields.
    // The "-1" is because the current merge counts as one target column.
    let max_fields_to_merge_for_current_target = original_fields.len()
        .saturating_sub(current_field_index)
        .saturating_sub(expected_types.len().saturating_sub(target_col_index).saturating_sub(1));

    if max_fields_to_merge_for_current_target == 0 { // Should not happen if previous checks are right, but as safeguard
        return false;
    }


    for num_fields_to_merge in 1..=max_fields_to_merge_for_current_target {
        let end_merge_index = current_field_index + num_fields_to_merge;

        // Slice the fields to be merged.
        let fields_to_join = &original_fields[current_field_index..end_merge_index];
        let merged_field_candidate_str = fields_to_join.join(delimiter_str);

        if is_field_type_compatible(
            &merged_field_candidate_str,
            &expected_types[target_col_index],
            decimal_separator,
        ) {
            fixed_line_so_far.push(merged_field_candidate_str);
            if try_merge_fields(
                original_fields,
                end_merge_index, // Next starting field index in original
                target_col_index + 1, // Next target column
                expected_types,
                decimal_separator,
                delimiter_str,
                fixed_line_so_far,
            ) {
                return true; // Solution found
            }
            fixed_line_so_far.pop(); // Backtrack
        }
    }

    false // No solution found for this path
}


fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Delimiter logic for csv crate - needed for both inference and main processing
    let delimiter_u8 = if args.delimiter == "\\t" {
        b'\t'
    } else {
        args.delimiter.as_bytes().first().copied().unwrap_or(b',')
    };

    let inferred_column_types: Vec<ColumnType> = if args.inference_lines > 0 {
        println!("Inférence des types de colonnes sur les {} premières lignes...", args.inference_lines);
        match infer_column_types(
            &args.file,
            &args.encoding,
            delimiter_u8,
            args.expected_fields,
            args.inference_lines,
            &args.decimal_separator,
        ) {
            Ok(types) => {
                if types.is_empty() { // Should not happen if inference_lines > 0, but good to check
                    eprintln!("L'inférence de type a renvoyé un vecteur vide, utilisation de Text par défaut pour toutes les colonnes.");
                    vec![ColumnType::Text; args.expected_fields]
                } else {
                    types
                }
            }
            Err(e) => {
                eprintln!("Erreur durant l'inférence des types: {}. Utilisation de Text par défaut pour toutes les colonnes.", e);
                vec![ColumnType::Text; args.expected_fields]
            }
        }
    } else {
        vec![ColumnType::Text; args.expected_fields]
    };

    // dbg!(&inferred_column_types); // Commented out as per requirement

    let input_file = File::open(&args.file)?;
    let initial_reader = BufReader::new(input_file);

    let encoding_obj_val = match args.encoding.to_lowercase().as_str() { // Renamed 'encoding_obj' to 'encoding_obj_val'
        "utf-8" => encoding_rs::UTF_8,
        "windows-1252" | "iso-8859-1" => encoding_rs::WINDOWS_1252, // Corrected mapping for iso-8859-1
        other => {
            eprintln!("Encodage non supporté: {other}, utilisation de utf-8 par défaut");
            encoding_rs::UTF_8
        }
    };

    let transcoded_reader = encoding_rs_io::DecodeReaderBytesBuilder::new()
        .encoding(Some(encoding_obj_val)) // Use renamed variable
        .build(initial_reader);

    let mut csv_reader = csv::ReaderBuilder::new()
        .delimiter(delimiter_u8) // Use pre-calculated delimiter_u8
        .has_headers(false)
        .from_reader(BufReader::new(transcoded_reader));

    let out_file = File::create(&args.output)?;
    let mut writer = BufWriter::new(out_file);

    let mut count = 0usize;
    let mut ok = 0usize;
    let mut fixed = 0usize;
    let mut bad = 0usize;
    let mut parse_error_count = 0usize; // New counter for CSV parsing errors
    // let mut progress_shown = false; // Removed for indicatif

    // Initialize ProgressBar
    let pb: ProgressBar;
    if let Some(max_val) = args.max {
        pb = ProgressBar::new(max_val as u64);
        pb.set_style(ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos}/{len} ({per_sec}, ETA: {eta})")
            .unwrap_or_else(|_| ProgressStyle::default_bar()) // Fallback style
            .progress_chars("#>-"));
    } else {
        pb = ProgressBar::new_spinner();
        pb.set_style(ProgressStyle::default_spinner()
            .template("{spinner:.green} [{elapsed_precise}] {pos} lines processed ({per_sec})")
            .unwrap_or_else(|_| ProgressStyle::default_spinner()));
    }

    for record_result in csv_reader.records() {
        let record = match record_result {
            Ok(r) => r,
            Err(e) => {
                parse_error_count += 1;
                bad +=1; 
                let error_line = format!("#ERROR (parsing error on line {}): {}", count + 1, e);
                if let Err(write_err) = writeln!(writer, "{}", error_line) {
                    eprintln!("Critical: Failed to write error marker for line {}: {}", count + 1, write_err);
                }
                // Ensure progress bar is handled even for errored lines before continue
                count += 1; 
                pb.inc(1);
                if let Some(max_lines) = args.max {
                    if count >= max_lines {
                        // No need for specific println! here, pb.finish_with_message will handle it
                        break;
                    }
                }
                continue; 
            }
        };
        let fields: Vec<String> = record.iter().map(String::from).collect();

        let line_to_write: String;

        if fields.len() == args.expected_fields {
            ok += 1;
            line_to_write = fields.join(&args.delimiter);
        } else if fields.len() > args.expected_fields {
            // Try intelligent merging if inference was active and successful
            if args.inference_lines > 0 && inferred_column_types.len() == args.expected_fields {
                let mut resolved_fields: Vec<String> = Vec::new();
                let success = try_merge_fields(
                    &fields,
                    0,
                    0,
                    &inferred_column_types,
                    &args.decimal_separator,
                    &args.delimiter, // Pass the original delimiter string
                    &mut resolved_fields,
                );

                if success && resolved_fields.len() == args.expected_fields {
                    fixed += 1;
                    line_to_write = resolved_fields.join(&args.delimiter);
                } else {
                    bad += 1;
                    let mut bad_line_fields = vec![format!(
                        "#BAD_MERGE_FAILED ({} champs, attendus {}, résolus {})",
                        fields.len(),
                        args.expected_fields,
                        resolved_fields.len()
                    )];
                    bad_line_fields.extend(fields.iter().cloned());
                    line_to_write = bad_line_fields.join(&args.delimiter);
                }
            } else {
                // Inference not active or types not suitable, use #BAD_EXCESS_NO_INFERENCE
                bad += 1;
                let mut bad_line_fields =
                    vec![format!("#BAD_EXCESS_NO_INFERENCE ({} champs)", fields.len())];
                bad_line_fields.extend(fields.iter().cloned());
                line_to_write = bad_line_fields.join(&args.delimiter);
            }
        } else { // fields.len() < args.expected_fields
            bad += 1;
            let mut bad_line_fields = vec![format!("#BAD_FEW ({} champs)", fields.len())];
            bad_line_fields.extend(fields.iter().cloned());
            line_to_write = bad_line_fields.join(&args.delimiter);
        }

        writeln!(writer, "{}", line_to_write)?;

        count += 1;
        pb.inc(1); // Increment progress bar

        // The old progress printing logic is removed.
        // if count % 100_000 == 0 {
        //     print!("\rLignes traitées : {count}");
        //     std::io::stdout().flush()?;
        //     progress_shown = true;
        // }

        if let Some(max_lines) = args.max {
            if count >= max_lines {
                 // Message moved to pb.finish_with_message
                break;
            }
        }
    }

    pb.finish_with_message("Processing complete."); // Generic finish message

    writer.flush()?;

    // New comprehensive summary
    println!("--------------------------------------------------");
    println!("Summary:");
    println!("--------------------------------------------------");
    println!("Total lines processed : {}", count);
    println!("Lines correct (OK)    : {}", ok);
    println!("Lines fixed           : {}", fixed);
    println!("Lines with parse errors: {} (Could not be fully parsed by CSV reader)", parse_error_count);
    println!("Lines marked as BAD   : {} (e.g., too few/many fields, merge failed post-parse)", bad - parse_error_count); // Subtract parse_error_count if they are double-counted in 'bad'
    println!("--------------------------------------------------");
    println!("Corrected file written to: {:?}", args.output);
    println!("--------------------------------------------------");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write; // For File::write_all

    // Helper function to create temporary CSV files for testing
    fn create_temp_csv(content: &str, file_name_prefix: &str) -> PathBuf {
        let mut i = 0;
        loop {
            let file_name = format!("{}_{}_{}.csv", file_name_prefix, std::process::id(), i);
            let mut path = std::env::temp_dir();
            path.push(&file_name);
            if !path.exists() {
                let mut file = File::create(&path).unwrap_or_else(|e| {
                    panic!("Failed to create temp file {:?}: {}", path, e)
                });
                file.write_all(content.as_bytes()).unwrap_or_else(|e| {
                    panic!("Failed to write to temp file {:?}: {}", path, e)
                });
                return path;
            }
            i += 1; // Try next index if file already exists (e.g. from a previous interrupted test run)
        }
    }

    // --- Tests for infer_column_types ---

    #[test]
    fn test_infer_all_numeric_point_decimal() {
        let csv_content = "10,20.5,300
1,2.0,30
0,0.0,0";
        let temp_file = create_temp_csv(csv_content, "infer_all_numeric_point");
        let types = infer_column_types(&temp_file, "utf-8", b',', 3, 10, ".").unwrap();
        assert_eq!(types, vec![ColumnType::Numeric, ColumnType::Numeric, ColumnType::Numeric]);
        std::fs::remove_file(temp_file).unwrap();
    }

    #[test]
    fn test_infer_all_numeric_comma_decimal() {
        let csv_content = "10;20,5;300
1;2,0;30
0;0,0;0";
        let temp_file = create_temp_csv(csv_content, "infer_all_numeric_comma");
        let types = infer_column_types(&temp_file, "utf-8", b';', 3, 10, ",").unwrap();
        assert_eq!(types, vec![ColumnType::Numeric, ColumnType::Numeric, ColumnType::Numeric]);
        std::fs::remove_file(temp_file).unwrap();
    }

    #[test]
    fn test_infer_all_text() {
        let csv_content = "a,b,c
d,e,f
g,h,i";
        let temp_file = create_temp_csv(csv_content, "infer_all_text");
        let types = infer_column_types(&temp_file, "utf-8", b',', 3, 10, ".").unwrap();
        assert_eq!(types, vec![ColumnType::Text, ColumnType::Text, ColumnType::Text]);
        std::fs::remove_file(temp_file).unwrap();
    }

    #[test]
    fn test_infer_mixed_types() {
        let csv_content = "10,hello,20.5,true
1,world,30,,
,system,1.0,false"; // Added an empty string in 2nd line, 4th col
        let temp_file = create_temp_csv(csv_content, "infer_mixed");
        let types = infer_column_types(&temp_file, "utf-8", b',', 4, 10, ".").unwrap();
        assert_eq!(types, vec![ColumnType::Numeric, ColumnType::Text, ColumnType::Numeric, ColumnType::Text]);
        std::fs::remove_file(temp_file).unwrap();
    }

    #[test]
    fn test_infer_empty_cols_become_text() {
        let csv_content = "a,,c
d,,f
g,,i";
        let temp_file = create_temp_csv(csv_content, "infer_empty_cols");
        let types = infer_column_types(&temp_file, "utf-8", b',', 3, 10, ".").unwrap();
        // Empty columns are finalized to Text
        assert_eq!(types, vec![ColumnType::Text, ColumnType::Text, ColumnType::Text]);
        std::fs::remove_file(temp_file).unwrap();
    }
    
    #[test]
    fn test_infer_truly_empty_col_mixed_with_data() {
        let csv_content = "1,,data
2,,text
3,,info";
        let temp_file = create_temp_csv(csv_content, "infer_truly_empty_mixed");
        let types = infer_column_types(&temp_file, "utf-8", b',', 3, 10, ".").unwrap();
        assert_eq!(types, vec![ColumnType::Numeric, ColumnType::Text, ColumnType::Text]);
        std::fs::remove_file(temp_file).unwrap();
    }


    #[test]
    fn test_infer_max_lines_zero() {
        let csv_content = "1,text
2,another";
        let temp_file = create_temp_csv(csv_content, "infer_max_lines_zero");
        let types = infer_column_types(&temp_file, "utf-8", b',', 2, 0, ".").unwrap();
        assert!(types.is_empty()); // As per current implementation for 0 lines
        std::fs::remove_file(temp_file).unwrap();
    }

    #[test]
    fn test_infer_fewer_lines_than_max() {
        let csv_content = "1,text
2,another";
        let temp_file = create_temp_csv(csv_content, "infer_fewer_lines");
        let types = infer_column_types(&temp_file, "utf-8", b',', 2, 10, ".").unwrap();
        assert_eq!(types, vec![ColumnType::Numeric, ColumnType::Text]);
        std::fs::remove_file(temp_file).unwrap();
    }

    #[test]
    fn test_infer_skips_incorrect_field_count_lines() {
        let csv_content = "1,text,10.1
2,text
3,world,30.3,extra
4,test,40.4"; // This is the only 'good' line for 3 expected fields.
        let temp_file = create_temp_csv(csv_content, "infer_skip_bad_lines");
        // Expecting 3 fields, only line 4 has 3 fields.
        let types = infer_column_types(&temp_file, "utf-8", b',', 3, 10, ".").unwrap();
        assert_eq!(types, vec![ColumnType::Numeric, ColumnType::Text, ColumnType::Numeric]);
        std::fs::remove_file(temp_file).unwrap();
    }
    
    #[test]
    fn test_infer_numeric_becomes_text() {
        let csv_content = "1,10
a,20
3,30";
        let temp_file = create_temp_csv(csv_content, "infer_num_to_text");
        let types = infer_column_types(&temp_file, "utf-8", b',', 2, 10, ".").unwrap();
        assert_eq!(types, vec![ColumnType::Text, ColumnType::Numeric]);
        std::fs::remove_file(temp_file).unwrap();
    }

    // --- Tests for try_merge_fields ---

    fn s(st: &str) -> String { st.to_string() }
    fn sv(sv: Vec<&str>) -> Vec<String> { sv.iter().map(|s| s.to_string()).collect() }

    #[test]
    fn test_merge_simple_numeric() {
        let fields = sv(vec!["1", "23", "text"]);
        let expected_types = vec![ColumnType::Numeric, ColumnType::Text];
        let mut resolved = Vec::new();
        let success = try_merge_fields(&fields, 0, 0, &expected_types, ".", ",", &mut resolved);
        assert!(success);
        assert_eq!(resolved, sv(vec!["1,23", "text"]));
    }

    #[test]
    fn test_merge_simple_text() {
        let fields = sv(vec!["hello", "world", "123"]);
        let expected_types = vec![ColumnType::Text, ColumnType::Numeric];
        let mut resolved = Vec::new();
        let success = try_merge_fields(&fields, 0, 0, &expected_types, ".", ",", &mut resolved);
        assert!(success);
        assert_eq!(resolved, sv(vec!["hello,world", "123"]));
    }

    #[test]
    fn test_merge_no_valid_merge() {
        let fields = sv(vec!["text1", "123", "text2"]); // text1,123 cannot be numeric
        let expected_types = vec![ColumnType::Numeric, ColumnType::Text];
        let mut resolved = Vec::new();
        let success = try_merge_fields(&fields, 0, 0, &expected_types, ".", ",", &mut resolved);
        assert!(!success);
        assert!(resolved.is_empty()); // Should be empty as no solution found from the start
    }

    #[test]
    fn test_merge_multiple_merges() {
        let fields = sv(vec!["a", "b", "1", "2", "c", "d"]);
        let expected_types = vec![ColumnType::Text, ColumnType::Numeric, ColumnType::Text];
        let mut resolved = Vec::new();
        let success = try_merge_fields(&fields, 0, 0, &expected_types, ".", ",", &mut resolved);
        assert!(success);
        assert_eq!(resolved, sv(vec!["a,b", "1,2", "c,d"]));
    }
    
    #[test]
    fn test_merge_complex_scenario_abc12c() {
        let fields = sv(vec!["a", "b", "1", "2", "c"]);
        let expected_types = vec![ColumnType::Text, ColumnType::Numeric, ColumnType::Text];
        let mut resolved = Vec::new();
        let success = try_merge_fields(&fields, 0, 0, &expected_types, ".", ",", &mut resolved);
        assert!(success);
        assert_eq!(resolved, sv(vec!["a,b", "1,2", "c"]));
    }

    #[test]
    fn test_merge_with_empty_strings_as_part() {
        // Merge "text", "" into a Text field -> "text,"
        let fields = sv(vec!["text", "", "123"]);
        let expected_types = vec![ColumnType::Text, ColumnType::Numeric];
        let mut resolved = Vec::new();
        let success = try_merge_fields(&fields, 0, 0, &expected_types, ".", ",", &mut resolved);
        assert!(success);
        assert_eq!(resolved, sv(vec!["text,", "123"]));
    }

    #[test]
    fn test_merge_with_empty_string_as_full_field_compatible_numeric() {
        // Merge "" into a Numeric field -> "" (compatible)
        let fields = sv(vec!["", "actual_text"]);
        let expected_types = vec![ColumnType::Numeric, ColumnType::Text];
        let mut resolved = Vec::new();
        let success = try_merge_fields(&fields, 0, 0, &expected_types, ".", ",", &mut resolved);
        assert!(success);
        assert_eq!(resolved, sv(vec!["", "actual_text"]));
    }
    
    #[test]
    fn test_merge_producing_wrong_final_count_should_fail_overall_but_try_merge_may_succeed_partially() {
        // try_merge_fields is successful if it can make all target_types compatible by consuming original_fields.
        // If it consumes all original_fields but not all target_types, it fails.
        // If it consumes all target_types but not all original_fields, it fails.
        
        // Scenario 1: Consumes all target_types, but original_fields remain.
        let fields = sv(vec!["1", "2", "text", "extra"]); // Expected: Numeric, Text
        let expected_types = vec![ColumnType::Numeric, ColumnType::Text];
        let mut resolved = Vec::new();
        let success = try_merge_fields(&fields, 0, 0, &expected_types, ".", ",", &mut resolved);
        assert!(!success); // Fails because "extra" is not consumed.

        // Scenario 2: Consumes all original_fields, but target_types remain.
        let fields2 = sv(vec!["1", "2"]); // Expected: Numeric, Text, Numeric
        let expected_types2 = vec![ColumnType::Numeric, ColumnType::Text, ColumnType::Numeric];
        let mut resolved2 = Vec::new();
        let success2 = try_merge_fields(&fields2, 0, 0, &expected_types2, ".", ",", &mut resolved2);
        assert!(!success2); // Fails because the third expected type cannot be filled.
    }
    
    #[test]
    fn test_merge_single_field_to_multiple_targets_not_possible() {
        // This tests the constraint of max_fields_to_merge_for_current_target
        let fields = sv(vec!["a,b,c"]); // one original field
        let expected_types = vec![ColumnType::Text, ColumnType::Text]; // two target fields
        let mut resolved = Vec::new();
        let success = try_merge_fields(&fields, 0, 0, &expected_types, ".", ",", &mut resolved);
        assert!(!success);
    }

    // End-to-End tests are complex due to main's structure.
    // Acknowledging this limitation for this subtask.
    // Priority was given to unit tests for `infer_column_types` and `try_merge_fields`.
}
