//! AddonInfo 的定义与实现。

/// 一个 addon 的启用状态（`GetAddonsV2` 回 `a(sssibbbasas)`，这里只取用到的字段）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddonInfo {
    pub name: String,

    pub category: i32,

    pub enabled: bool,
}
