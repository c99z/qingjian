//! `#rrggbb` / `#rrggbbaa` 颜色串的解析与格式化（classicui 主题色字段的写法）。

/// 解析 `#rrggbb` 或 `#rrggbbaa`（忽略 alpha），不合法返回 `None`。
pub fn parse_hex(text: &str) -> Option<(u8, u8, u8)> {
    let hex = text.strip_prefix('#')?;
    if hex.len() != 6 && hex.len() != 8 {
        return None;
    }
    let channel = |at: usize| u8::from_str_radix(hex.get(at..at + 2)?, 16).ok();
    Some((channel(0)?, channel(2)?, channel(4)?))
}

pub fn to_hex(color: (u8, u8, u8)) -> String {
    format!("#{:02x}{:02x}{:02x}", color.0, color.1, color.2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hex_accepts_six_and_eight_digits() {
        assert_eq!(parse_hex("#164B73"), Some((0x16, 0x4b, 0x73)));
        assert_eq!(parse_hex("#ffffff00"), Some((0xff, 0xff, 0xff)));
        for bad in ["", "#", "#fff", "164B73", "#gggggg", "#12345"] {
            assert_eq!(parse_hex(bad), None, "{bad}");
        }
    }

    #[test]
    fn to_hex_is_lowercase_six_digits() {
        assert_eq!(to_hex((0x16, 0x4b, 0x73)), "#164b73");
        assert_eq!(parse_hex(&to_hex((1, 2, 3))), Some((1, 2, 3)));
    }
}
