//! ConfigValue 的定义与实现。

use gio::glib;
use gio::glib::prelude::*;

/// 一个字段的值。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigValue {
    Text(String),

    Bool(bool),

    Int(i64),

    Raw(glib::Variant),
}

impl ConfigValue {
    /// 从 `a{sv}` 的 `v` 里解出来；不认识的类型返回 `None`（调用方跳过，不拖垮整个读取）。
    pub fn from_variant(variant: &glib::Variant) -> Option<Self> {
        match variant.type_().as_str() {
            "s" => variant.get::<String>().map(Self::Text),
            "b" => variant.get::<bool>().map(Self::Bool),
            "i" => variant.get::<i32>().map(|v| Self::Int(v.into())),
            "u" => variant.get::<u32>().map(|v| Self::Int(v.into())),
            "x" => variant.get::<i64>().map(Self::Int),
            other => {
                let _ = other;
                Some(Self::Raw(variant.clone()))
            }
        }
    }

    pub fn to_variant(&self) -> glib::Variant {
        match self {
            Self::Text(text) => text.to_variant(),
            Self::Bool(on) => on.to_variant(),
            // fcitx5 的整数字段是 i32
            Self::Int(value) => i32::try_from(*value).unwrap_or(i32::MAX).to_variant(),
            Self::Raw(value) => value.clone(),
        }
    }

    /// 报错信息里给用户看的写法。
    pub fn describe(&self) -> String {
        match self {
            Self::Text(text) => text.clone(),
            Self::Bool(on) => if *on { "开" } else { "关" }.to_owned(),
            Self::Int(value) => value.to_string(),
            Self::Raw(value) => value.to_string(),
        }
    }
}
