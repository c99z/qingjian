//! 设置快照：一次读入配置文件原文、解析结果与文件版本，作为整个编辑会话的基准（base）。
//! 解析失败时快照仍可用——界面要能显示错误并继续编辑其他部分，`config` 此时是缺省值，仅供参考。

use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::error::ConfigError;

use super::file_version::FileVersion;
use super::resources::ResourceCapabilities;

#[derive(Debug, Clone)]
pub struct SettingsSnapshot {
    /// 配置文件路径。
    pub path: PathBuf,

    /// 文件原文；文件不存在为 `None`（保存时从模板起步）。
    pub source: Option<String>,

    /// 解析出的配置；解析失败时是缺省值，真正的错误在 `parse_error`。
    pub config: Config,

    /// 读取时的文件版本；文件不存在为 `None`。
    pub version: Option<FileVersion>,

    /// TOML 解析错误。
    pub parse_error: Option<String>,

    /// 资源能力（可用词库 / 释义语言 / 本地模型）；不带资源加载时为空。
    pub resources: ResourceCapabilities,
}

impl SettingsSnapshot {
    /// 只读配置文件，不枚举资源。
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let source = match std::fs::read_to_string(path) {
            Ok(source) => Some(source),
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => None,
            Err(source) => {
                return Err(ConfigError::Read {
                    path: path.to_owned(),
                    source,
                });
            }
        };
        let (config, parse_error) = match &source {
            Some(text) => match toml::from_str::<Config>(text) {
                Ok(config) => {
                    // 与 Config::load 一样把密钥登记给日志掩码
                    if let Some(key) = config.predict.resolve_api_key() {
                        crate::logs::secrets::register(&key);
                    }
                    (config, None)
                }
                Err(error) => (Config::default(), Some(error.to_string())),
            },
            None => (Config::default(), None),
        };
        let version = source.as_ref().map(|text| {
            let modified = std::fs::metadata(path).and_then(|m| m.modified()).ok();
            FileVersion::from_source(text, modified)
        });
        Ok(Self {
            path: path.to_owned(),
            source,
            config,
            version,
            parse_error,
            resources: ResourceCapabilities::default(),
        })
    }

    /// 读配置并枚举资源能力：`resource_root` 是随包资源根，`user_dir` 是用户数据目录。
    pub fn load_with_resources(
        path: &Path,
        resource_root: Option<&Path>,
        user_dir: &Path,
    ) -> Result<Self, ConfigError> {
        let mut snapshot = Self::load(path)?;
        snapshot.resources = ResourceCapabilities::discover(resource_root, user_dir);
        Ok(snapshot)
    }
}
