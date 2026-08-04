use crate::error::ErrorSignals;

const HEADER_IDENTIFIER: &str = "#";


const HEADER_PROTOCOL_DEF: &str = "protocol=";
const HEADER_PROTOCOL_DEF_LEN: uszie = HEADER_PROTOCOL_DEF.len();
const HEADER_IGNORE_DEF: &str = "ignore=";


enum Protocols {
    Can,
    Nil
}

impl From<String> for Protocols {
    fn from(value: String) -> Self {
        match value.to_lowercase() {
            String::from("can") => Self::Can,
            _ => Self::Nil
        }
    }
}


fn type_protocol(root_header: String) -> Result<Protocols, ErrorSignals> {
    if !root_header.starts_with(HEADER_IDENTIFIER) {
        return Err(ErrorSignals::InvalidHeader);
    }

    let header_cnt = &root_header[1..];
    if HEADER_IGNORE_DEF == header_cnt {
        return Err(ErrorSignals::NoError);
    }

    if HEADER_PROTOCOL_DEF != header_cnt[..HEADER_PROTOCOL_DEF_LEN] {
        return Err(ErrorSignals::UnidentifiableHeader);
    }


    let header_val = header_cnt[HEADER_PROTOCOL_DEF_LEN..];

    return header_val.into();
}