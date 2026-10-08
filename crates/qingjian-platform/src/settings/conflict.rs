//! 保存冲突：草稿改过的字段同时被外部改成了别的值。整体拒绝写入，由界面展示后让用户重新载入。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldConflict {
    /// 字段路径，如 `general.page_size`；自定义短语整体替换是 `custom_phrases`。
    pub field: String,

    /// 快照里的值（TOML 原文写法）；当时字段不存在为 `None`。
    pub base: Option<String>,

    /// 当前文件里的值；字段不存在或读不出来为 `None`。
    pub current: Option<String>,

    /// 草稿想写入的值。
    pub draft: String,
}
