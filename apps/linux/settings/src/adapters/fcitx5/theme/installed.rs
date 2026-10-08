//! InstalledTheme 的定义与实现。

/// 一个已安装主题（来自 classicui 配置的 Theme 字段描述，含本地化显示名）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledTheme {
    pub name: String,

    pub label: String,
}
