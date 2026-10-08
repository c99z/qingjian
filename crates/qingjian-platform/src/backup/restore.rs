//! 文件恢复事务；验证全部正文后替换，调用方重新加载内存成功才提交。
use super::{Archive, BackupPaths, DATA_FILES};
use crate::credentials;
use base64::{Engine, prelude::BASE64_STANDARD};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub struct RestoreGuard {
    original: BTreeMap<PathBuf, Option<Vec<u8>>>,

    committed: bool,
}
fn check_path(path: &Path) -> Result<(), String> {
    for ancestor in path.ancestors() {
        if ancestor.is_symlink() {
            return Err(format!("恢复目标经过符号链接：{}", ancestor.display()));
        }
    }
    if path.exists() && !path.is_file() {
        return Err(format!("恢复目标不是普通文件：{}", path.display()));
    }
    Ok(())
}
impl RestoreGuard {
    pub fn prepare(archive: &Archive, paths: &BackupPaths) -> Result<Self, String> {
        archive.validate()?;
        let stage = tempfile::tempdir().map_err(|e| e.to_string())?;
        let mut replacements = BTreeMap::new();
        // 先验证所有自定义词库，避免写到一半才发现坏 .qj。
        for (logical, data) in &archive.files {
            let mut decoded = BASE64_STANDARD
                .decode(data)
                .map_err(|_| "归档正文损坏".to_owned())?;
            if logical.starts_with("data/dicts/") {
                let path = stage
                    .path()
                    .join(Path::new(logical).file_name().ok_or("词库名称无效")?);
                std::fs::write(&path, &decoded).map_err(|e| e.to_string())?;
                let dictionary = qingjian_dictionary::Dictionary::from_path(&path)
                    .map_err(|_| format!("归档词库损坏：{logical}"))?;
                if dictionary.is_empty() {
                    return Err(format!("归档词库为空：{logical}"));
                }
            }
            if logical == "config.toml" {
                // 旧的显式密钥保留在本机；归档本身永远不带。
                let mut doc: toml_edit::DocumentMut = std::str::from_utf8(&decoded)
                    .map_err(|e| e.to_string())?
                    .parse()
                    .map_err(|e: toml_edit::TomlError| e.to_string())?;
                if let Ok(current) = crate::Config::load(&paths.config)
                    && let Some(key) = current.predict.api_key
                    && !key.is_empty()
                {
                    doc["predict"]["api_key"] = toml_edit::value(key);
                }
                decoded = doc.to_string().into_bytes();
            }
            replacements.insert(paths.target(logical), Some(decoded));
        }
        for file in DATA_FILES {
            replacements.entry(paths.data.join(file)).or_insert(None);
        }
        for (_, path) in crate::extra_dictionaries::list(&paths.data.join("dicts")) {
            replacements.entry(path).or_insert(None);
        }
        // 非优先格式的同名 TSV 也移除，恢复的是完整快照。
        if let Ok(entries) = std::fs::read_dir(paths.data.join("dicts")) {
            for entry in entries {
                let path = entry.map_err(|e| e.to_string())?.path();
                if path.extension().is_some_and(|e| e == "qj" || e == "tsv") {
                    replacements.entry(path).or_insert(None);
                }
            }
        }
        collect_theme_removals(
            &paths.themes.join("qingjian-custom"),
            "themes/qingjian-custom",
            &mut replacements,
        )?;
        let mut original = BTreeMap::new();
        for path in replacements.keys() {
            check_path(path)?;
            let bytes = match std::fs::read(path) {
                Ok(bytes) => Some(bytes),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                Err(e) => return Err(e.to_string()),
            };
            original.insert(path.clone(), bytes);
        }
        let mut guard = Self {
            original,
            committed: false,
        };
        for (path, data) in replacements {
            let result = match data {
                Some(bytes) => credentials::write_private(&path, &bytes),
                None => remove(&path),
            };
            if let Err(error) = result {
                guard.rollback()?;
                return Err(format!("恢复文件失败，已回滚：{error}"));
            }
        }
        Ok(guard)
    }
    pub fn commit(mut self) {
        self.committed = true;
    }
    pub fn rollback(&mut self) -> Result<(), String> {
        let mut failures = Vec::new();
        for (path, data) in &self.original {
            let result = match data {
                Some(data) => credentials::write_private(path, data),
                None => remove(path),
            };
            if let Err(error) = result {
                failures.push(error.to_string());
            }
        }
        self.committed = failures.is_empty();
        if failures.is_empty() {
            Ok(())
        } else {
            Err(format!("恢复回滚失败：{}", failures.join("；")))
        }
    }
}
fn collect_theme_removals(
    dir: &Path,
    logical: &str,
    replacements: &mut BTreeMap<PathBuf, Option<Vec<u8>>>,
) -> Result<(), String> {
    if dir.is_symlink() {
        return Err("恢复主题经过符号链接".into());
    }
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.to_string()),
    };
    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let name = format!("{logical}/{}", entry.file_name().to_string_lossy());
        if !super::allowed(&name) {
            continue;
        }
        let path = entry.path();
        if path.is_symlink() {
            return Err("恢复主题包含符号链接".into());
        }
        if path.is_dir() {
            collect_theme_removals(&path, &name, replacements)?;
        } else {
            replacements.entry(path).or_insert(None);
        }
    }
    Ok(())
}
fn remove(path: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        result => result,
    }
}
impl Drop for RestoreGuard {
    fn drop(&mut self) {
        if !self.committed
            && let Err(error) = self.rollback()
        {
            tracing::error!(%error, "恢复回滚失败");
        }
    }
}
