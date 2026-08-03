use strum_macros::EnumIter;

trait t_KeyWord {
   fn any(s: String) -> Option<Self>;
}


#[derive(EnumIter, Debug, Clone)]
enum KeyWords {
    Type,
    Def,
    Hash,
    Dot,
    FrameDef,
    Frame,
}

#[derive(EnumIter, Debug, Clone)]
enum FrameKeyWords {
    Size,
    Id,
    Fd,
    SizeMark
}


impl From<KeyWords> for &str {
    fn from(value: KeyWords) -> Self {
        match value {
            Type => "type",
            Def => "def",
            Hash => "#"
            Dot => "."
            FrameDef => "framedef"
            Frame => "frame"
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
            "framedef" => FrameDef
            "frame" => Frame
        }
    }
}


impl From<FrameKeyWords> for &str {
    fn from(value: KeyWords) -> Self {
        match value {
            Size => "size",
            Id => "id",
            Fd => "fd"
            SizeMark => "'"
        }
    }
}


impl t_KeyWord for KeyWords {
    fn any(s: String) -> Option<Self> {

    }
}

impl t_KeyWord for FrameKeyWords {
    fn any(s: String) -> Option<Self> {
    }
}