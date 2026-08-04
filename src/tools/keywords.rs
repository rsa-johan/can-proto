use strum_macros::EnumIter;

pub trait AnyKeyWord {
   fn any(s: String) -> Option<Self>;
}


#[derive(EnumIter, Debug, Clone)]
pub enum KeyWords {
    Type,
    Def,
    Hash,
    Dot,
    FrameDef,
    Frame,
}

#[derive(EnumIter, Debug, Clone)]
pub enum FrameKeyWords {
    Size,
    Id,
    Fd,
    SizeMark
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
        }
    }
}

impl From<&str> for KeyWords {
    fn from(value: &str) -> Self {
        match value {
            "type" => Type,
            "def" => Def,
            "#" => Hash,
            "." => Dot,
            "framedef" => FrameDef,
            "frame" => Frame
        }
    }
}


impl From<FrameKeyWords> for &str {
    fn from(value: FrameKeyWords) -> Self {
        match value {
            FrameKeyWords::Size => "size",
            FrameKeyWords::Id => "id",
            FrameKeyWords::Fd => "fd",
            FrameKeyWords::SizeMark => "'"
        }
    }
}