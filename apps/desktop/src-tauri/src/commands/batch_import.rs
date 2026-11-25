use std::io::Read;
use std::path::{Component, Path};
use tauri::command;

use edm_core::batch_import::{parse_batch_import, ImportedLink};

const MAX_BATCH_IMPORT_BYTES: u64 = 5 * 1024 * 1024; // 5 MiB

#[command]
pub fn parse_batch_import_file(file_path: String) -> Result<Vec<ImportedLink>, String> {
    let trimmed = file_path.trim();
    if trimmed.is_empty() {
        return Err("File path cannot be empty".into());
    }

    let path = Path::new(trimmed);
    if path.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err("Invalid file path: parent directory traversal not allowed".into());
    }

    let canonical = std::fs::canonicalize(path)
        .map_err(|e| format!("Failed to resolve batch import file: {}", e))?;
    if !canonical.is_file() {
        return Err("Target path is not a regular file".into());
    }

    let metadata = std::fs::metadata(&canonical)
        .map_err(|e| format!("Failed to read file metadata: {}", e))?;
    if metadata.len() > MAX_BATCH_IMPORT_BYTES {
        return Err("Batch import file exceeds 5 MiB size limit".into());
    }

    let format = canonical
        .extension()
        .and_then(|ext| ext.to_str())
        .ok_or_else(|| {
            "Batch import file must have a .txt, .csv, or .html extension".to_string()
        })?;

    let file = std::fs::File::open(&canonical)
        .map_err(|e| format!("Failed to open batch import file: {}", e))?;
    let mut limited = file.take(MAX_BATCH_IMPORT_BYTES + 1);
    let mut content = String::new();
    limited
        .read_to_string(&mut content)
        .map_err(|e| format!("Failed to read UTF-8 batch import file: {}", e))?;

    if content.len() as u64 > MAX_BATCH_IMPORT_BYTES {
        return Err("Batch import file exceeds 5 MiB size limit".into());
    }

    parse_batch_import(&content, format)
}

#[command]
pub fn parse_batch_import_text(
    content: String,
    format: String,
) -> Result<Vec<ImportedLink>, String> {
    if content.len() as u64 > MAX_BATCH_IMPORT_BYTES {
        return Err("Batch import content exceeds 5 MiB size limit".into());
    }
    parse_batch_import(&content, &format)
}
