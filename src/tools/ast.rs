use std::{collections::HashMap, ffi::OsStr};

use super::keywords::{KeyWords, FrameKeyWords};

struct Link {
   field_name: String,
   field_value: String,
   field_type: String,
   field_link: Box<Link>
}

struct Ast {
    top: HashMap<FrameKeyWords, Link>,
    root: Link
}