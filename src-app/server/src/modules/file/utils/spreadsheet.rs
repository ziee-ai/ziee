// Spreadsheet utilities for text extraction and CSV conversion

use calamine::{open_workbook, Ods, Reader, Xls, Xlsx};
use std::io::Write;
use std::path::Path;

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn escape_csv_cell(cell_str: &str) -> String {
    // Escape CSV special characters
    if cell_str.contains(',') || cell_str.contains('"') || cell_str.contains('\n') {
        format!("\"{}\"", cell_str.replace("\"", "\"\""))
    } else {
        cell_str.to_string()
    }
}

/// Convert XLSX file to CSV format and return the content as a string
/// Each sheet is separated by a header with the sheet name
pub fn convert_xlsx_to_text(
    file_path: &Path,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let mut workbook: Xlsx<_> = open_workbook(file_path)?;
    let mut content = String::new();

    for (sheet_index, sheet_name) in workbook.sheet_names().iter().enumerate() {
        if let Ok(range) = workbook.worksheet_range(sheet_name) {
            if sheet_index > 0 {
                content.push_str("\n\n");
            }
            content.push_str(&format!("=== Sheet: {} ===\n", sheet_name));

            for row in range.rows() {
                let csv_row: Vec<String> = row
                    .iter()
                    .map(|cell| escape_csv_cell(&format!("{}", cell)))
                    .collect();
                content.push_str(&format!("{}\n", csv_row.join(",")));
            }
        }
    }

    Ok(content)
}

/// Convert XLS file to CSV format and return the content as a string
/// Each sheet is separated by a header with the sheet name
pub fn convert_xls_to_text(
    file_path: &Path,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let mut workbook: Xls<_> = open_workbook(file_path)?;
    let mut content = String::new();

    for (sheet_index, sheet_name) in workbook.sheet_names().iter().enumerate() {
        if let Ok(range) = workbook.worksheet_range(sheet_name) {
            if sheet_index > 0 {
                content.push_str("\n\n");
            }
            content.push_str(&format!("=== Sheet: {} ===\n", sheet_name));

            for row in range.rows() {
                let csv_row: Vec<String> = row
                    .iter()
                    .map(|cell| escape_csv_cell(&format!("{}", cell)))
                    .collect();
                content.push_str(&format!("{}\n", csv_row.join(",")));
            }
        }
    }

    Ok(content)
}

/// Convert ODS file to CSV format and return the content as a string
/// Each sheet is separated by a header with the sheet name
pub fn convert_ods_to_text(
    file_path: &Path,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let mut workbook: Ods<_> = open_workbook(file_path)?;
    let mut content = String::new();

    for (sheet_index, sheet_name) in workbook.sheet_names().iter().enumerate() {
        if let Ok(range) = workbook.worksheet_range(sheet_name) {
            if sheet_index > 0 {
                content.push_str("\n\n");
            }
            content.push_str(&format!("=== Sheet: {} ===\n", sheet_name));

            for row in range.rows() {
                let csv_row: Vec<String> = row
                    .iter()
                    .map(|cell| escape_csv_cell(&format!("{}", cell)))
                    .collect();
                content.push_str(&format!("{}\n", csv_row.join(",")));
            }
        }
    }

    Ok(content)
}

/// Convert XLSX file to separate CSV files in a temporary directory
/// Returns a vector of paths to the created CSV files
pub fn convert_xlsx_to_csv_files(
    file_path: &Path,
    temp_dir: &Path,
) -> Result<Vec<std::path::PathBuf>, Box<dyn std::error::Error + Send + Sync>> {
    let mut workbook: Xlsx<_> = open_workbook(file_path)?;
    let mut csv_files = Vec::new();

    for (sheet_index, sheet_name) in workbook.sheet_names().iter().enumerate() {
        if let Ok(range) = workbook.worksheet_range(sheet_name) {
            let csv_filename = format!(
                "sheet_{}_{}.csv",
                sheet_index + 1,
                sanitize_filename(sheet_name)
            );
            let csv_path = temp_dir.join(&csv_filename);

            let mut csv_file = std::fs::File::create(&csv_path)?;

            for row in range.rows() {
                let csv_row: Vec<String> = row
                    .iter()
                    .map(|cell| escape_csv_cell(&format!("{}", cell)))
                    .collect();
                writeln!(csv_file, "{}", csv_row.join(","))?;
            }

            csv_files.push(csv_path);
        }
    }

    Ok(csv_files)
}

/// Convert XLS file to separate CSV files in a temporary directory
/// Returns a vector of paths to the created CSV files
pub fn convert_xls_to_csv_files(
    file_path: &Path,
    temp_dir: &Path,
) -> Result<Vec<std::path::PathBuf>, Box<dyn std::error::Error + Send + Sync>> {
    let mut workbook: Xls<_> = open_workbook(file_path)?;
    let mut csv_files = Vec::new();

    for (sheet_index, sheet_name) in workbook.sheet_names().iter().enumerate() {
        if let Ok(range) = workbook.worksheet_range(sheet_name) {
            let csv_filename = format!(
                "sheet_{}_{}.csv",
                sheet_index + 1,
                sanitize_filename(sheet_name)
            );
            let csv_path = temp_dir.join(&csv_filename);

            let mut csv_file = std::fs::File::create(&csv_path)?;

            for row in range.rows() {
                let csv_row: Vec<String> = row
                    .iter()
                    .map(|cell| escape_csv_cell(&format!("{}", cell)))
                    .collect();
                writeln!(csv_file, "{}", csv_row.join(","))?;
            }

            csv_files.push(csv_path);
        }
    }

    Ok(csv_files)
}

/// Convert ODS file to separate CSV files in a temporary directory
/// Returns a vector of paths to the created CSV files
pub fn convert_ods_to_csv_files(
    file_path: &Path,
    temp_dir: &Path,
) -> Result<Vec<std::path::PathBuf>, Box<dyn std::error::Error + Send + Sync>> {
    let mut workbook: Ods<_> = open_workbook(file_path)?;
    let mut csv_files = Vec::new();

    for (sheet_index, sheet_name) in workbook.sheet_names().iter().enumerate() {
        if let Ok(range) = workbook.worksheet_range(sheet_name) {
            let csv_filename = format!(
                "sheet_{}_{}.csv",
                sheet_index + 1,
                sanitize_filename(sheet_name)
            );
            let csv_path = temp_dir.join(&csv_filename);

            let mut csv_file = std::fs::File::create(&csv_path)?;

            for row in range.rows() {
                let csv_row: Vec<String> = row
                    .iter()
                    .map(|cell| escape_csv_cell(&format!("{}", cell)))
                    .collect();
                writeln!(csv_file, "{}", csv_row.join(","))?;
            }

            csv_files.push(csv_path);
        }
    }

    Ok(csv_files)
}
