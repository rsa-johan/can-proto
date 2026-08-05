use strum_macros::EnumIter;


#[derive(EnumIter, Debug, Clone)]
pub enum KeyWords {
    Type,
    Def,
    Hash,
    Dot,
    FrameDef,
    Frame,
    Nil
}

#[derive(EnumIter, Debug, Clone)]
pub enum FrameKeyWords {
    Size,
    Id,
    Fd,
    SizeMark,
    Nil
}


impl From<KeyWords> for &str {
    fn from(value: KeyWords) -> Self {
        match value {
            KeyWords::Type => "type",
            KeyWords::Def => "def",
            KeyWords::Hash => "#",
            KeyWords::Dot => ".",
            KeyWords::FrameDef => "framedef",
            KeyWords::Frame => "frame",
            KeyWords::Nil => "nil",
        }
    }
}

impl From<&str> for KeyWords {
    fn from(value: &str) -> Self {
        match value.to_lowercase().as_str() {
            "type" => KeyWords::Type,
            "def" => KeyWords::Def,
            "#" => KeyWords::Hash,
            "." => KeyWords::Dot,
            "framedef" => KeyWords::FrameDef,
            "frame" => KeyWords::Frame,
            _ => KeyWords::Nil
        }
    }
}


impl From<FrameKeyWords> for &str {
    fn from(value: FrameKeyWords) -> Self {
        match value {
            FrameKeyWords::Size => "size",
            FrameKeyWords::Id => "id",
            FrameKeyWords::Fd => "fd",
            FrameKeyWords::SizeMark => "'",
            FrameKeyWords::Nil => "nil",
        }
    }
}

impl From<&str> for FrameKeyWords {
    fn from(value: &str) -> Self {
        match value.to_lowercase().as_str() {
            "size" => FrameKeyWords::Size,
            "id" => FrameKeyWords::Id,
            "fd" => FrameKeyWords::Fd,
            "'" => FrameKeyWords::SizeMark,
            _ => FrameKeyWords::Nil
        }
    }
}