use std::{fs, path::Path};

use crate::error::ErrorSignals;


const ROOT_FILE_NAME: &str = ".root";

fn root_check(c_dir: Path) -> Result<_, ErrorSignals>{
    if !c_dir.is_dir() {
        return Err(ErrorSignals::NotDir)
    }

    let found_root = false;

    for item in fs::read_dir(c_dir).map_err(|e| ErrorSignals::Generic(e.to_string()))? {
        if item.is_err() {
            continue;
        }
        let item = item.unwrap();
        if item.is_file() {
            let name = item.file_stem();
            if name.is_empty() { continue; }
        }
    }
}
