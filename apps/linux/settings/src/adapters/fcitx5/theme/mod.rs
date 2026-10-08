//! 经典面板主题：已装主题的目录定位、定制主题的复制与 `theme.conf` 改写。
//!
//! 选已有主题时原主题一个字节都不动；要改释义颜色 / 字号比例这类主题字段时，把源主题整个目录
//! （含图片资源）复制到青简管理的 [`CUSTOM_THEME_NAME`]，再只改副本的 `theme.conf`。

mod installed;
pub use installed::InstalledTheme;
mod patch;
pub use patch::ThemePatch;
mod style;
pub use style::CommentStyle;
mod option;
pub use option::ThemeOption;

use std::path::PathBuf;

/// 青简管理的定制主题名（用户主题目录下的目录名）。
pub const CUSTOM_THEME_NAME: &str = "qingjian-custom";

/// 用户主题目录 `$XDG_DATA_HOME/fcitx5/themes`（缺省 `~/.local/share/fcitx5/themes`），不负责创建。
pub fn user_themes_dir() -> PathBuf {
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("."));
    data.join("fcitx5/themes")
}

/// 系统主题目录；fcitx5 还会看 `$XDG_DATA_DIRS` 里的其他前缀，这里只覆盖常见两个。
fn system_themes_dirs() -> Vec<PathBuf> {
    std::env::var_os("XDG_DATA_DIRS")
        .unwrap_or_else(|| "/usr/local/share:/usr/share".into())
        .to_string_lossy()
        .split(':')
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .map(|path| path.join("fcitx5/themes"))
        .collect()
}

/// 按名字找主题目录：用户目录优先（与 fcitx5 的查找顺序一致）。
pub fn find_theme_dir(name: &str) -> Option<PathBuf> {
    if !valid_theme_name(name) {
        return None;
    }
    std::iter::once(user_themes_dir())
        .chain(system_themes_dirs())
        .map(|dir| dir.join(name))
        .find(|dir| dir.join("theme.conf").is_file())
}

pub fn valid_theme_name(name: &str) -> bool {
    !name.is_empty() && name != "." && name != ".." && !name.contains(['/', '\\', '\n', '\r'])
}

mod transaction;
pub use transaction::ThemeTransaction;

/// 改写 `theme.conf` 文本：`[Metadata]` 整段换成定制主题的身份信息，
/// `[InputPanel]` 里覆盖 / 补上 [`ThemePatch`] 给出的键，其余行原样保留。
pub fn patch_theme_conf(source: &str, patch: &ThemePatch) -> String {
    use std::collections::BTreeMap;
    const METADATA: &str = "[Metadata]\nName=Qingjian Custom\nName[zh_CN]=青简定制\nVersion=1\nAuthor=Qingjian\nDescription=青简设置生成的定制主题\n";
    let mut updates: BTreeMap<&str, BTreeMap<&str, String>> = BTreeMap::new();
    for (key, value) in [
        ("NormalColor", &patch.normal_color),
        ("HighlightCandidateColor", &patch.highlight_color_text),
    ] {
        if let Some(value) = value {
            updates
                .entry("InputPanel")
                .or_default()
                .insert(key, value.clone());
        }
    }
    if let Some(factor) = patch.comment_size_factor {
        updates
            .entry("InputPanel")
            .or_default()
            .insert("CommentTextSizeFactor", factor.to_string());
    }
    // Optional|Color 不是主节字符串；Fcitx5 只认子节中的 Value（marshallfunction.h）。
    for (section, color) in [
        ("InputPanel/CandidateCommentColor", &patch.comment_color),
        (
            "InputPanel/HighlightCandidateCommentColor",
            &patch.highlight_comment_color,
        ),
    ] {
        if let Some(color) = color {
            updates
                .entry(section)
                .or_default()
                .insert("Value", color.clone());
        }
    }
    let mut remaining = updates.clone();
    let mut out = METADATA.to_owned();
    let mut section = String::new();
    let flush = |section: &str,
                 remaining: &mut BTreeMap<&str, BTreeMap<&str, String>>,
                 out: &mut String| {
        if let Some(fields) = remaining.remove(section) {
            for (key, value) in fields {
                out.push_str(&format!("{key}={value}\n"));
            }
        }
    };
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            flush(&section, &mut remaining, &mut out);
            section = trimmed.trim_matches(['[', ']']).to_owned();
            if section != "Metadata" {
                out.push_str(line);
                out.push('\n');
            }
            continue;
        }
        if section == "Metadata" {
            continue;
        }
        if let Some((key, _)) = trimmed.split_once('=') {
            let key = key.trim();
            if section == "InputPanel"
                && ((key == "CandidateCommentColor" && patch.comment_color.is_some())
                    || (key == "HighlightCandidateCommentColor"
                        && patch.highlight_comment_color.is_some()))
            {
                continue;
            }
            if updates
                .get(section.as_str())
                .is_some_and(|fields| fields.contains_key(key))
            {
                if let Some(value) = remaining
                    .get_mut(section.as_str())
                    .and_then(|fields| fields.remove(key))
                {
                    out.push_str(&format!("{key}={value}\n"));
                }
                continue;
            }
        }
        out.push_str(line);
        out.push('\n');
    }
    flush(&section, &mut remaining, &mut out);
    for (section, fields) in remaining {
        if fields.is_empty() {
            continue;
        }
        out.push_str(&format!("\n[{section}]\n"));
        for (key, value) in fields {
            out.push_str(&format!("{key}={value}\n"));
        }
    }
    out
}

pub fn theme_options(themes: &[InstalledTheme], current: &str) -> Vec<ThemeOption> {
    let label_of = |name: &str| {
        themes
            .iter()
            .find(|theme| theme.name == name)
            .map(|theme| theme.label.clone())
            .unwrap_or_else(|| name.to_owned())
    };
    let mut options = vec![ThemeOption {
        label: format!("保持当前主题（{}）", label_of(current)),
        name: None,
    }];
    for (preset, label) in [
        ("default", "浅色预设（默认主题）"),
        ("default-dark", "深色预设（默认深色主题）"),
    ] {
        if themes.iter().any(|theme| theme.name == preset) {
            options.push(ThemeOption {
                label: label.to_owned(),
                name: Some(preset.to_owned()),
            });
        }
    }
    for theme in themes {
        if theme.name == "default" || theme.name == "default-dark" {
            continue;
        }
        options.push(ThemeOption {
            label: theme.label.clone(),
            name: Some(theme.name.clone()),
        });
    }
    options
}

#[cfg(test)]
mod tests;
