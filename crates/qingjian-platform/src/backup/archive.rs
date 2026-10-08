//! 版本化用户数据归档，不含凭据、日志和模型。
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Archive {
    pub version: u32,

    pub created_at: u64,

    /// 相对逻辑路径到 Base64 正文，路径必须通过共享白名单验证。
    pub files: BTreeMap<String, String>,

    pub checksums: BTreeMap<String, String>,
}
