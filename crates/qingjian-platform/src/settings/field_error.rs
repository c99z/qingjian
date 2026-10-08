//! 草稿校验报出的单个字段错误：字段路径 + 给用户看的中文说明。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldError {
    /// 字段路径，如 `general.page_size`。
    pub field: String,

    /// 给用户看的中文说明。
    pub message: String,
}

impl FieldError {
    pub fn new(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
        }
    }
}
