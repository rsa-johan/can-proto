use std::{fs, path::Path};

use crate::{common::PROTO_EXT, error::ErrorSignals};


const ROOT_FILE_NAME: &str = ".root";

pub fn root_check(c_dir: &Path) -> Result<String, ErrorSignals>{
    if !c_dir.is_dir() {
        return Err(ErrorSignals::NotDir)
    }

    for item in fs::read_dir(c_dir).map_err(|e| ErrorSignals::Generic(e.to_string()))? {
        if item.is_err() {
            continue;
        }
        let item = item.unwrap().path();
        if !item.is_file() { continue; }

        if let Some(name) = item.file_stem().and_then(|s| s.to_str()) {
            if let Some(ext) = item.extension().and_then(|s| s.to_str()) {
                if is_root_proto_file(name, ext) {
                    return fs::read_to_string(item).map_err(|e| ErrorSignals::Generic(e.to_string()));
                }
            }
        }
    }
    Err(ErrorSignals::RootFileNotFound)
}

pub fn fetch_header(h_sect: String) -> Result<(), ErrorSignals> {
    todo!()
}



#[inline]
fn is_root_proto_file(name: &str, ext: &str) -> bool {
    ROOT_FILE_NAME == name && PROTO_EXT == ext 
}