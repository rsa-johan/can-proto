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

#[allow(dead_code)]
enum ParseKeys {
    Equals,
    Semicolon,
    Default,
    DefaultDebug,
    LeftBracket,
    RightBracket,
    LeftCurly,
    RightCurly,
    Struct,
    NewLine,
    Space,
    Enum,
    Debug,
    Comma,
    Implement,
    From,
    FromCapitalized,
    For,
    Function,
    Return,
    Constructor,
    Public,
    LeftAngle,
    RightAngle,
}

impl From<ParseKeys> for &str {
    fn from(value: ParseKeys) -> Self {
        match value {
            ParseKeys::Debug => "#[derive(Debug)]",
            ParseKeys::Default => "#[derive(Default)]",
            ParseKeys::DefaultDebug => "#[derive(Default, Debug)]",
            ParseKeys::Equals => "=",
            ParseKeys::Semicolon => ";",
            ParseKeys::LeftBracket => "(",
            ParseKeys::RightBracket => ")",
            ParseKeys::LeftCurly => "{",
            ParseKeys::RightCurly => "}",
            ParseKeys::Struct => "struct",
            ParseKeys::Space => " ",
            #[cfg(target_os = "windows")]
            ParseKeys::NewLine => "\r\n",
            #[cfg(not(target_os = "windows"))]
            ParseKeys::NewLine => "\n",
            ParseKeys::Enum => "enum",
            ParseKeys::Comma => ",",
            ParseKeys::Implement => "impl",
            ParseKeys::From => "from",
            ParseKeys::FromCapitalized => "From",
            ParseKeys::For => "for",
            ParseKeys::Function => "fn",
            ParseKeys::Constructor => "new",
            ParseKeys::Return => "return",
            ParseKeys::LeftAngle => "<",
            ParseKeys::RightAngle => ">",
            ParseKeys::Public => "pub",
        }
    }
}

#[derive(Debug)]
struct Variable {
    name: String,
    value: Option<String> 
}

impl From<&str> for Variable {
    fn from(value: &str) -> Self {
        println!("Parsing variable from string: '{}'", value);
        let val = value.trim().to_string();
        let mut parts: Vec<String> = val.trim().splitn(2, '=').map(|e| e.to_string()).collect();
        let value = parts.pop();
        let name = parts.pop();
        match name {
            Some(val) => Variable { name: val, value },
            None => panic!("No name found within")
        }
    }
}


#[derive(Debug)]
enum KeyWords {
    Enum(KeyValue),
    Byte(KeyValue),
    Frame(KeyValue),
    Id(KeyValue),
    ByteCol(KeyValue),
    Reserved(String),
    None
}

#[derive(Default, Debug)]
struct KeyValue {
    key: String,
    value: Vec<Variable>,
}

impl KeyValue {
    fn validate_value(part: &str) -> String {
        let len: usize = part.len();
        if part.chars().nth(0).unwrap() != '{' || part.chars().nth(len - 1).unwrap() != '}' {
            panic!("Value is not in expected pattern")
        }
        part.strip_prefix('{').unwrap().strip_suffix('}').unwrap().trim().to_string()
    }
}

impl From<&str> for KeyValue {
    fn from(value: &str) -> Self {
        let val = value.to_string();
        println!("Parsing value: {}", val);
        let parts: Vec<&str> = val.splitn(2, '=').collect();
        if parts.len() != 2 {
            panic!("Invalid key-value format, expected 'key+value'");
        }
        let key = parts[0].trim().to_string();
        let validated_value = KeyValue::validate_value(parts[1].trim());
        let value: Vec<Variable> = validated_value.split(";").filter_map(|e| { 
            if e.trim().is_empty() {
                None
            } else {
                Some(e.into())
            }
         }).collect();
        println!("Parsed key: '{}', value: {:#?}", key, value);
        
        KeyValue {
            key,
            value,
        }
    }
}

impl From<&str> for KeyWords {
    fn from(value: &str) -> Self {
        if "Enum_t" == value {
            KeyWords::Enum(KeyValue::default())
        } else if "Byte" == value {
            KeyWords::Byte(KeyValue::default())
        } else if "Frame" == value {
            KeyWords::Frame(KeyValue::default())
        } else if "ID" == value {
            KeyWords::Id(KeyValue::default())
        } else if "ByteCol" == value {
            KeyWords::ByteCol(KeyValue::default())
        } else if "Reserved" == value {
            KeyWords::Reserved(value.to_string())
        } else {
            KeyWords::None
        }
    }
}

impl From<KeyWords> for String {
    fn from(value: KeyWords) -> Self {
        match value {
            KeyWords::Enum(s) => KeyWords::parse_enum(s),
            KeyWords::Byte(s) => KeyWords::parse_byte(s),
            KeyWords::Frame(s) => KeyWords::parse_frame(s),
            KeyWords::Id(s) => KeyWords::parse_id(s),
            KeyWords::ByteCol(s) => KeyWords::parse_byte_col(s),
            KeyWords::Reserved(s) => s,
            KeyWords::None => String::new(),
        }
    }
}

impl KeyWords {
    fn parse_from_token(&self, s: &str) -> Self {
        match self {
            KeyWords::Enum(_) => KeyWords::Enum(s.into()),
            KeyWords::Id(_) => KeyWords::Id(s.into()),
            KeyWords::Byte(_) => KeyWords::Byte(s.into()),
            KeyWords::ByteCol(_) => KeyWords::ByteCol(s.into()),
            KeyWords::Reserved(_) => KeyWords::Reserved(s.into()),
            KeyWords::Frame(_) => KeyWords::Frame(s.into()),
            _ => panic!("not a proper proto file;  Syntax error!!")
        }
    }

    #[allow(dead_code)]
    fn is_none(&self) -> bool {
        matches!(self, KeyWords::None)
    }
}

impl KeyWords {
    fn parse_enum(s : KeyValue) -> String {
        let mut result: Vec<&str>= Vec::new();
        #[cfg(feature = "debug")]
        result.push(ParseKeys::Debug.into());

        let enum_def = utils::enum_template(&s);
        let impl_u8 = utils::impl_u8_template(&s);

        result.push(&enum_def);
        result.push(&impl_u8);
        result.push(ParseKeys::NewLine.into());
        result.join(ParseKeys::NewLine.into())
    }

    fn parse_byte(s : KeyValue) -> String {
        let mut result: Vec<&str>= Vec::new();
        #[cfg(feature = "debug")]
        result.push(ParseKeys::DefaultDebug.into());
        #[cfg(not(feature = "debug"))]
        result.push(ParseKeys::Default.into());

        let enum_def = utils::struct_template(&s);
        let impl_u8 = utils::impl_parse_template(&s);

        result.push(&enum_def);
        result.push(&impl_u8);
        result.push(ParseKeys::NewLine.into());
        result.join(ParseKeys::NewLine.into())
    }

    fn parse_byte_col(s : KeyValue) -> String {
        let mut result: Vec<&str>= Vec::new();
        #[cfg(feature = "debug")]
        result.push(ParseKeys::Debug.into());

        let struct_def = utils::struct_template(&s);
        let impl_u8 = utils::impl_u8_template(&s);

        result.push(&struct_def);
        result.push(&impl_u8);
        result.join(ParseKeys::NewLine.into())
    }

    fn parse_frame(s : KeyValue) -> String {
        let mut result: Vec<&str>= Vec::new();
        #[cfg(feature = "debug")]
        result.push(ParseKeys::Debug.into());

        let struct_def = utils::struct_template(&s);
        let impl_u8 = utils::impl_u8_template(&s);

        result.push(&struct_def);
        result.push(&impl_u8);
        result.join(ParseKeys::NewLine.into())
    }

    fn parse_id(s : KeyValue) -> String {
        let mut result: Vec<&str>= Vec::new();
        #[cfg(feature = "debug")]
        result.push(ParseKeys::Debug.into());

        let enum_def = utils::enum_template(&s);
        let impl_u8 = utils::impl_u8_template(&s);

        result.push(&enum_def);
        result.push(&impl_u8);
        result.join(ParseKeys::NewLine.into())
    }
}


fn compile_proto(proto_content: TokenStream) -> TokenStream {
    if proto_content.is_empty() {
        panic!("Proto content is empty");
    }

    let mut key_words: Vec<KeyWords> = Vec::new();
    let mut temp = String::new();
    proto_content.clone().into_iter().for_each(|e| println!("Token: {}", e));
    for token in proto_content.into_iter().map(|e| e.to_string()) {
        let key_word = KeyWords::from(token.as_ref());
        println!("Processing token: '{}', identified as: {:#?}", token, key_word);
        if let KeyWords::None = key_word {
            if let Some(_) = key_words.last_mut() {
                println!("Appending token to temp: '{}'", token);
                temp.push_str(&token[..]);
                temp.push(' ');
            } else {
                println!("No active keyword to associate with token: '{}'", token);
                if !temp.is_empty() {
                    println!("Appending token to temp in else block: '{}'", token);
                    temp.push_str(&token[..]);
                } else {
                    println!("No active keyword to associate with token: '{}', and temp is empty. Ignoring token.", token);
                    panic!("Invalid token: {}", token);
                }
            }
        } else {
            println!("Identified new keyword: {:#?}, pushing to key_words vector", key_word);
            if !temp.is_empty() {
                if let Some(lst_kw) = key_words.last_mut() {
                    println!("Parsing temp for last keyword: '{:#?}' with temp content: '{}'", lst_kw, temp);
                    let new_kw = lst_kw.parse_from_token(&temp[..]);
                    println!("Updated last keyword: {:#?}", new_kw);
                    *lst_kw = new_kw;
                    println!("Clearing temp after parsing");
                    temp.clear();
                }
            }
            key_words.push(key_word);
        }
    }

    if !temp.is_empty() {
        if let Some(lst_kw) = key_words.last_mut() {
            println!("Parsing temp for last keyword: '{:#?}' with temp content: '{}'", lst_kw, temp);
            let new_kw = lst_kw.parse_from_token(&temp[..]);
            println!("Updated last keyword: {:#?}", new_kw);
            *lst_kw = new_kw;
            println!("Clearing temp after parsing");
            temp.clear();
        }
    }
    
    println!("Parsed key words: {:#?}", key_words);
    let code = key_words.into_iter().fold(String::new(), |acc, kw| acc + &String::from(kw));
    println!("Generated code:\n{}", code);
    code.parse().expect("Unable to parse generated code")
}
