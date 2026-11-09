//! 整理回复只能选择本机提供的编号，不能创造词条、读音或频率。

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OrganizationReply {
    pub accepted: Vec<usize>,

    pub domains: Vec<String>,
}

impl OrganizationReply {
    pub fn parse(text: &str, size: usize) -> Result<Self, String> {
        let reply: Self = serde_json::from_str(text).map_err(|_| "整理回复格式不合法")?;
        let mut ids = reply.accepted.clone();
        ids.sort_unstable();
        ids.dedup();
        if ids.len() != reply.accepted.len()
            || ids.iter().any(|i| *i >= size)
            || reply.domains.len() > 8
            || reply
                .domains
                .iter()
                .any(|d| d.is_empty() || d.chars().count() > 24 || d.chars().any(char::is_control))
        {
            return Err("整理回复含无效编号或领域".into());
        }
        Ok(reply)
    }
}
