//! ThemeOption 的定义与实现。

/// 主题下拉框的选项：第一项「保持当前主题」，然后浅色 / 深色预设（fcitx5 自带主题，装了才列），
/// 最后是其余已装主题。`name` 为 `None` 表示不动当前主题。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeOption {
    pub label: String,

    pub name: Option<String>,
}
