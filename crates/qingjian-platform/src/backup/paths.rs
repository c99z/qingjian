//! 归档逻辑根在本机的映射，服务只使用已装配的目录。
use std::path::PathBuf;
#[derive(Clone)]
pub struct BackupPaths {
    pub config: PathBuf,

    pub data: PathBuf,

    pub themes: PathBuf,
}
impl BackupPaths {
    pub fn local() -> Self {
        let data = crate::dirs::linux::user_dir();
        Self {
            config: crate::dirs::linux::config_path(),
            themes: data.parent().unwrap_or(&data).join("fcitx5/themes"),
            data,
        }
    }
    pub(super) fn target(&self, logical: &str) -> PathBuf {
        if logical == "config.toml" {
            self.config.clone()
        } else if let Some(path) = logical.strip_prefix("data/") {
            self.data.join(path)
        } else {
            self.themes
                .join(logical.strip_prefix("themes/").unwrap_or(logical))
        }
    }
}
