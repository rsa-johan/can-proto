mod protocols;
mod common;
mod tools;
mod error;
mod init;

use std::{path::Path};

use proc_macro::TokenStream;

use crate::init::{root_check, fetch_header};

fn compile_proto(_proto_content: TokenStream) {
   todo!()
}

fn compile(p_dir: &'static str) -> Result<(), error::ErrorSignals> {
    let root_content = root_check(Path::new(p_dir))?; 
    fetch_header(root_content);
    Ok(())
}