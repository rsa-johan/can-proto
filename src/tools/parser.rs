use proc_macro::TokenStream;

pub fn parse(content: TokenStream) { 
    for token in content.into_iter() {
        println!("Token: {:#?}", token.to_string());
    }
}