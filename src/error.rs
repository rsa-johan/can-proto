#[derive(Debug)]
pub enum ErrorSignals {
    Generic(String),
    NotDir,

    InvalidHeader,

    UnidentifiableHeader,


    NoError
}


// impl From<ErrorSignals> for String {
//     fn from(value: ErrorSignals) -> Self {
//         match value {
//             ErrorSignals::InvalidHeader => "Invalid root file header found".to_string(),
//             ErrorSignals::UnidentifiableHeader => "UnidentifiableHeader".to_string(),
//             ErrorSignals::NotDir => "NotDir".to_string()
//             _ => "nil".to_string()
//         }
//     }
// }