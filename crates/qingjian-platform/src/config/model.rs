//! 本地模型开关；语境排序尚在验收阶段，旧配置默认关闭。

use serde::{Deserialize, Serialize};

/// 配置文件 `[model]` 分节：本地整句模型的开关。
///
/// 随包的字级小模型在本机给整句候选重新排序，全程离线、不联网，与云联想互不影响（本地先出、云端到了另占它自己的格）。
/// 模型文件不在包里（或用户目录 `model/` 里）时开关无效。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct LocalModelConfig {
    /// 开着就加载模型、给整句重排。
    pub enabled: bool,

    /// 在本机用最近输入给中文候选增加有限语境分。
    pub context_ranking: bool,
}

impl Default for LocalModelConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            context_ranking: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::LocalModelConfig;

    #[test]
    fn old_configuration_does_not_enable_context() {
        let old: LocalModelConfig = toml::from_str("enabled = true").unwrap();
        assert!(old.enabled);
        assert!(!old.context_ranking);
        let new: LocalModelConfig =
            toml::from_str("enabled = false\ncontext_ranking = true").unwrap();
        assert!(!new.enabled);
        assert!(new.context_ranking);
        assert_eq!(
            toml::from_str::<LocalModelConfig>(&toml::to_string(&new).unwrap()).unwrap(),
            new
        );
    }
}
