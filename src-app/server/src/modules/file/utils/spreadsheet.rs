// Spreadsheet utilities for text extraction and CSV conversion

use calamine::{open_workbook, Ods, Reader, Xls, Xlsx};
use std::path::Path;

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

