//! FieldMismatch 的定义与实现。

/// 读回比对的一个不一致字段。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldMismatch {
    /// 给用户看的中文字段名。
    pub label: &'static str,

    pub expected: String,

    pub actual: String,
}
