//! 实机验证前备份经典面板与青简定制主题，所有验证路径都恢复原值。

use std::path::{Path, PathBuf};

use qingjian_linux_settings::adapters::fcitx5::{ConfigMap, Fcitx5Client};

pub struct Backup {
    config: ConfigMap,

    theme: PathBuf,

    files: tempfile::TempDir,

    had_theme: bool,
}

impl Backup {
    pub fn new(client: &Fcitx5Client) -> Result<Self, String> {
        let config = client.classic_ui_map().map_err(|e| e.user_message())?;
        let base = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(std::env::var_os("HOME").unwrap()).join(".local/share")
            });
        let theme = base.join("fcitx5/themes/qingjian-custom");
        let files = tempfile::tempdir().map_err(|e| e.to_string())?;
        let had_theme = theme.exists();
        if had_theme {
            copy_dir(&theme, &files.path().join("theme")).map_err(|e| e.to_string())?;
        }
        Ok(Self {
            config,
            theme,
            files,
            had_theme,
        })
    }

    pub fn restore(self, client: &Fcitx5Client) -> Result<(), String> {
        let restore = (|| {
            if self.theme.exists() {
                std::fs::remove_dir_all(&self.theme).map_err(|e| e.to_string())?;
            }
            if self.had_theme {
                copy_dir(&self.files.path().join("theme"), &self.theme)
                    .map_err(|e| e.to_string())?;
            }
            client
                .set_classic_ui(&self.config)
                .and_then(|()| client.reload_addon("classicui"))
                .map_err(|e| e.user_message())?;
            let readback = client.classic_ui_map().map_err(|e| e.user_message())?;
            if self.config != readback {
                return Err("恢复后配置与原值不一致".to_owned());
            }
            Ok(())
        })();
        if restore.is_err() {
            eprintln!("恢复失败，主题备份保留于 {}", self.files.keep().display());
        }
        restore
    }
}

fn copy_dir(source: &Path, target: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(target)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &target.join(entry.file_name()))?;
        } else {
            std::fs::copy(entry.path(), target.join(entry.file_name()))?;
        }
    }
    Ok(())
}
