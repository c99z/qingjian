//! Pango 字体描述串（classicui `Font`，形如 `Noto Sans CJK SC Bold 12`）的解析与拼装。
//! 只认「字族 + 可选 Bold + 尾部字号」；设置界面只产出常规 / 粗体，读回比对也只核对这三项。

mod weight;
pub use weight::FontWeight;
mod description;
pub use description::FontDesc;

/// 读回比对用：两边都按 [`FontDesc::parse`] 归一后比较；有解析不出来的就退回原文比较。
pub fn fonts_match(expected: &str, actual: &str) -> bool {
    FontDesc::parse(expected) == FontDesc::parse(actual)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_common_spellings() {
        assert_eq!(
            FontDesc::parse("Sans 10"),
            FontDesc {
                family: "Sans".to_owned(),
                weight: FontWeight::Regular,
                size: 10,
            }
        );
        assert_eq!(
            FontDesc::parse("Noto Sans CJK SC Bold 12"),
            FontDesc {
                family: "Noto Sans CJK SC".to_owned(),
                weight: FontWeight::Bold,
                size: 12,
            }
        );
        // 没有字号 / 完全解析不了都有缺省
        assert_eq!(FontDesc::parse("Serif").size, 10);
        assert_eq!(FontDesc::parse("").family, "Sans");
    }

    #[test]
    fn compose_roundtrips() {
        for text in ["Sans 10", "WenQuanYi Micro Hei Bold 14"] {
            assert_eq!(FontDesc::parse(text).compose(), text);
        }
    }

    #[test]
    fn fonts_match_normalizes_spacing_and_case() {
        assert!(fonts_match("Sans  Bold   10", "Sans Bold 10"));
        assert!(!fonts_match("Sans 10", "Sans 11"));
        assert!(!fonts_match("Sans Bold 10", "Sans 10"));
        assert!(!fonts_match("Serif 10", "Sans 10"));
    }
}
