//! 定制主题事务：同目录准备新副本，交换后保留原目录，D-Bus 确认成功才删除回滚副本。

use std::path::{Path, PathBuf};

use crate::adapters::fcitx5::error::Fcitx5Error;
use crate::adapters::fcitx5::theme::{
    CUSTOM_THEME_NAME, ThemePatch, find_theme_dir, patch_theme_conf, user_themes_dir,
};

pub struct ThemeTransaction {
    target: PathBuf,

    staging: tempfile::TempDir,

    had_original: bool,

    committed: bool,
}

impl ThemeTransaction {
    pub fn prepare(source: Option<&str>, patch: &ThemePatch) -> Result<Self, Fcitx5Error> {
        let target = user_themes_dir().join(CUSTOM_THEME_NAME);
        let source = match source {
            Some(name) => {
                find_theme_dir(name).ok_or_else(|| Fcitx5Error::ThemeNotFound(name.to_owned()))?
            }
            None => target.clone(),
        };
        Self::at(&source, &target, patch)
    }

    pub(super) fn at(
        source: &Path,
        target: &Path,
        patch: &ThemePatch,
    ) -> Result<Self, Fcitx5Error> {
        let io_error = |source| Fcitx5Error::ThemeIo {
            path: target.to_owned(),
            source,
        };
        let parent = target.parent().expect("主题目录有父目录");
        std::fs::create_dir_all(parent).map_err(io_error)?;
        let staging = tempfile::Builder::new()
            .prefix(".qingjian-theme-")
            .tempdir_in(parent)
            .map_err(io_error)?;
        let prepared = staging.path().join("prepared");
        copy_dir(source, &prepared).map_err(io_error)?;
        let conf = prepared.join("theme.conf");
        let text = std::fs::read_to_string(&conf).map_err(io_error)?;
        std::fs::write(&conf, patch_theme_conf(&text, patch)).map_err(io_error)?;
        let had_original = target.try_exists().map_err(io_error)?;
        if had_original {
            std::fs::rename(target, staging.path().join("original")).map_err(io_error)?;
        }
        let transaction = Self {
            target: target.to_owned(),
            staging,
            had_original,
            committed: false,
        };
        std::fs::rename(&prepared, target).map_err(io_error)?;
        Ok(transaction)
    }

    pub fn commit(mut self) {
        self.committed = true;
    }

    pub fn rollback(mut self) -> Result<(), Fcitx5Error> {
        let result = self.restore();
        // 恢复失败保留副本供修复，不能由 Drop 再次删除目标目录。
        self.committed = true;
        if result.is_err() {
            let path = std::mem::replace(
                &mut self.staging,
                tempfile::tempdir().map_err(|source| Fcitx5Error::ThemeIo {
                    path: self.target.clone(),
                    source,
                })?,
            )
            .keep();
            tracing::error!(backup = %path.display(), "主题回滚失败，已保留原主题副本");
        }
        result
    }

    fn restore(&self) -> Result<(), Fcitx5Error> {
        let io_error = |source| Fcitx5Error::ThemeIo {
            path: self.target.clone(),
            source,
        };
        if self.target.exists() {
            std::fs::remove_dir_all(&self.target).map_err(io_error)?;
        }
        if self.had_original {
            std::fs::rename(self.staging.path().join("original"), &self.target)
                .map_err(io_error)?;
        }
        Ok(())
    }
}

impl Drop for ThemeTransaction {
    fn drop(&mut self) {
        if !self.committed
            && let Err(error) = self.restore()
        {
            tracing::error!(%error, "恢复主题失败");
        }
    }
}

fn copy_dir(source: &Path, target: &Path) -> std::io::Result<()> {
    // 跟随合法的图片链接，但拒绝目录链接，避免循环或递归复制源目录之外的树。
    if std::fs::symlink_metadata(source)?.file_type().is_symlink() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "theme directory is a symlink",
        ));
    }
    std::fs::create_dir_all(target)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let from = entry.path();
        let to = target.join(entry.file_name());
        if from.is_dir() {
            copy_dir(&from, &to)?;
        } else if from.is_file() {
            std::fs::copy(&from, &to)?;
        } else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "unsupported theme file",
            ));
        }
    }
    Ok(())
}
