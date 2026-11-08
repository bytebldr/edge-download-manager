use std::path::{Path, PathBuf};
use tauri::{command, State};
use uuid::Uuid;

use crate::features;
use crate::services::download_manager::execute_download;
use crate::state::AppState;

#[command]
pub fn expand_batch_urls(pattern: String) -> Result<Vec<String>, String> {
    edm_core::batch::expand_batch_pattern(&pattern)
}

#[command]
pub async fn add_batch_downloads(
    state: State<'_, AppState>,
    urls: Vec<String>,
    category: Option<String>,
    save_dir: Option<String>,
) -> Result<usize, String> {
    let mut added = 0;
    for url in urls {
        let u = url.trim();
        if u.is_empty() {
            continue;
        }
        let id = Uuid::new_v4();
        let s_dir = save_dir.as_ref().map(PathBuf::from);
        if execute_download(
            &state,
            id,
            u.to_string(),
            None,
            s_dir,
            None,
            None,
            None,
            None,
            category.clone(),
        )
        .is_ok()
        {
            added += 1;
        }
    }
    Ok(added)
}

#[command]
pub fn check_duplicate_download(
    state: State<'_, AppState>,
    url: String,
    filename: String,
    size: Option<u64>,
) -> Result<features::DuplicateCheckResult, String> {
    match state.store.find_duplicate(&url, &filename, size) {
        Ok(Some(d)) => Ok(features::DuplicateCheckResult {
            is_duplicate: true,
            existing_id: Some(d.id.to_string()),
            existing_filename: Some(d.filename),
            existing_state: Some(format!("{:?}", d.state).to_lowercase()),
        }),
        _ => Ok(features::DuplicateCheckResult {
            is_duplicate: false,
            existing_id: None,
            existing_filename: None,
            existing_state: None,
        }),
    }
}

#[command]
pub fn verify_file_checksum(
    file_path: String,
    expected_hash: String,
    algo: String,
) -> Result<features::ChecksumResult, String> {
    features::verify_file_checksum(Path::new(&file_path), &expected_hash, &algo)
}
