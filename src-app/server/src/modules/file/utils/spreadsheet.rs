// Spreadsheet utilities for text extraction and CSV conversion

use calamine::{open_workbook, Ods, Reader, Xls, Xlsx};
use std::path::Path;

fn escape_csv_cell(cell_str: &str) -> String {
    // Defuse CSV-injection / formula-injection: when the cell starts
    // with `=`, `+`, `-`, `@`, TAB, or CR, prepend a single quote so
    // downstream spreadsheet apps (Excel / LibreOffice / Sheets)
    // render it as text instead of evaluating it as a formula like
    // `=cmd|"/c calc"!A1`. Closes 05-file F-21 (Low).
    let neutralised = match cell_str.chars().next() {
        Some('=') | Some('+') | Some('-') | Some('@') | Some('\t') | Some('\r') => {
            format!("'{}", cell_str)
        }
        _ => cell_str.to_string(),
    };

    // Escape CSV special characters
    if neutralised.contains(',') || neutralised.contains('"') || neutralised.contains('\n') {
        format!("\"{}\"", neutralised.replace("\"", "\"\""))
    } else {
        neutralised
    }
}

/// Convert XLSX file to per-sheet text (one string per sheet)
pub fn convert_xlsx_to_pages(
    file_path: &Path,
) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
    let mut workbook: Xlsx<_> = open_workbook(file_path)?;
    let mut pages = Vec::new();

    for sheet_name in workbook.sheet_names() {
        if let Ok(range) = workbook.worksheet_range(&sheet_name) {
            let mut content = String::new();
            content.push_str(&format!("=== Sheet: {} ===\n", sheet_name));

            for row in range.rows() {
                let csv_row: Vec<String> = row
                    .iter()
                    .map(|cell| escape_csv_cell(&format!("{}", cell)))
                    .collect();
                content.push_str(&format!("{}\n", csv_row.join(",")));
            }

            pages.push(content);
        }
    }

    Ok(pages)
}

/// Convert XLS file to per-sheet text (one string per sheet)
pub fn convert_xls_to_pages(
    file_path: &Path,
) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
    let mut workbook: Xls<_> = open_workbook(file_path)?;
    let mut pages = Vec::new();

    for sheet_name in workbook.sheet_names() {
        if let Ok(range) = workbook.worksheet_range(&sheet_name) {
            let mut content = String::new();
            content.push_str(&format!("=== Sheet: {} ===\n", sheet_name));

            for row in range.rows() {
                let csv_row: Vec<String> = row
                    .iter()
                    .map(|cell| escape_csv_cell(&format!("{}", cell)))
                    .collect();
                content.push_str(&format!("{}\n", csv_row.join(",")));
            }

            pages.push(content);
        }
    }

    Ok(pages)
}

/// Convert ODS file to per-sheet text (one string per sheet)
pub fn convert_ods_to_pages(
    file_path: &Path,
) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
    let mut workbook: Ods<_> = open_workbook(file_path)?;
    let mut pages = Vec::new();

    for sheet_name in workbook.sheet_names() {
        if let Ok(range) = workbook.worksheet_range(&sheet_name) {
            let mut content = String::new();
            content.push_str(&format!("=== Sheet: {} ===\n", sheet_name));

            for row in range.rows() {
                let csv_row: Vec<String> = row
                    .iter()
                    .map(|cell| escape_csv_cell(&format!("{}", cell)))
                    .collect();
                content.push_str(&format!("{}\n", csv_row.join(",")));
            }

            pages.push(content);
        }
    }

    Ok(pages)
}

