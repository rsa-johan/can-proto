mod utils;

use std::{path::Path};

use proc_macro::TokenStream;

#[proc_macro]
pub fn include_can_proto(path: TokenStream) -> TokenStream {
    let file_path = path.to_string().trim_matches('"').to_string();
    let proto_path = Path::new(&file_path);
    let proto_content: String = std::fs::read_to_string(proto_path).expect("Unable to read file").lines().filter(|line| !line.starts_with("#")).collect();
    println!("Proto content:\n{}", proto_content);
    println!("Beginning compilation...");
    let compiled_proto = compile_proto(proto_content.parse().expect("Unable to parse file content"));
    println!("Compilation completed successfully!");
    compiled_proto
}


fn compile_proto(proto_content: TokenStream) -> TokenStream {
    
}
