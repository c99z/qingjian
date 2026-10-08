//! FontWeight 的定义与实现。

/// 字重：界面只给常规 / 粗体两档。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontWeight {
    Regular,

    Bold,
}

impl FontWeight {
    pub const ALL: [Self; 2] = [Self::Regular, Self::Bold];

    pub fn label(self) -> &'static str {
        match self {
            Self::Regular => "常规",
            Self::Bold => "粗体",
        }
    }
}
