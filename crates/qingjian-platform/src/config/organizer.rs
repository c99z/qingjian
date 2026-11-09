//! 夜间个人词库整理；独立于逐键云联想，复用其接口与本机凭据。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct OrganizerConfig {
    pub enabled: bool,

    pub hour: u8,

    pub minute: u8,

    /// 用户补充的专业领域；留空时由模型只根据候选词判断。
    pub domain_hint: String,
}

impl Default for OrganizerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            hour: 3,
            minute: 0,
            domain_hint: String::new(),
        }
    }
}
