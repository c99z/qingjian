//! FontDesc 的定义与实现。

use crate::adapters::fcitx5::font::FontWeight;

/// 拆解后的字体描述。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontDesc {
    pub family: String,

    pub weight: FontWeight,

    pub size: u32,
}

impl Default for FontDesc {
    /// classicui 的出厂缺省是 `Sans 10`。
    fn default() -> Self {
        Self {
            family: "Sans".to_owned(),
            weight: FontWeight::Regular,
            size: 10,
        }
    }
}

impl FontDesc {
    /// 解析 Pango 字体描述串；解析不了的部分按缺省处理（字号缺 10、字重常规）。
    pub fn parse(text: &str) -> Self {
        let mut tokens: Vec<&str> = text.split_whitespace().collect();
        let mut size = None;
        if let Some(last) = tokens.last()
            && let Ok(value) = last.parse::<u32>()
            && value > 0
        {
            size = Some(value);
            tokens.pop();
        }
        let weight = if tokens.iter().any(|t| t.eq_ignore_ascii_case("bold")) {
            tokens.retain(|t| !t.eq_ignore_ascii_case("bold"));
            FontWeight::Bold
        } else {
            FontWeight::Regular
        };
        let family = tokens.join(" ");
        Self {
            family: if family.is_empty() {
                "Sans".to_owned()
            } else {
                family
            },
            weight,
            size: size.unwrap_or(10),
        }
    }

    /// 拼回 Pango 字体描述串。
    pub fn compose(&self) -> String {
        match self.weight {
            FontWeight::Regular => format!("{} {}", self.family, self.size),
            FontWeight::Bold => format!("{} Bold {}", self.family, self.size),
        }
    }
}
