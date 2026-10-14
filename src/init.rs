use std::{fs, path::{Path, PathBuf}};

use crate::{common::PROTO_EXT, error::ErrorSignals};

/// Proto files (`*.cp`, including `*.inc.cp`) of a directory, sorted; dotfiles are skipped.
pub fn collect_dir(c_dir: &Path) -> Result<Vec<PathBuf>, ErrorSignals> {
    if !c_dir.is_dir() {
        return Err(ErrorSignals::NotDir);
    }
    let mut files = Vec::new();
    for item in fs::read_dir(c_dir).map_err(|e| ErrorSignals::Generic(e.to_string()))? {
        let Ok(item) = item else { continue };
        let path = item.path();
        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
        if path.is_file() && ext == PROTO_EXT && !name.starts_with('.') {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}
