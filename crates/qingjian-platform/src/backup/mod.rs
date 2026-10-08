//! 本机一致性快照的归档格式、路径白名单和可回滚恢复。调用方协调内存写入。
mod archive;
mod paths;
mod restore;
use crate::{Config, credentials};
pub use archive::Archive;
use base64::{Engine, prelude::BASE64_STANDARD};
pub use paths::BackupPaths;
pub use restore::RestoreGuard;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Component, Path};

pub const ARCHIVE_VERSION: u32 = 1;
pub const MAX_ARCHIVE_BYTES: usize = 96 * 1024 * 1024;
pub const MAX_DATA_BYTES: usize = 64 * 1024 * 1024;
pub const DATA_FILES: &[&str] = &[
    "user.tsv",
    "user-words.tsv",
    "user-ngram.tsv",
    "user-choices.tsv",
    "user-english.tsv",
    "user-typos.tsv",
    "usage.tsv",
    "user-vocab.tsv",
    "user-glossary-en.tsv",
    "user-glossary-ja.tsv",
    "user-glossary-es.tsv",
    "user-glossary-zh.tsv",
];

pub(super) fn allowed(name: &str) -> bool {
    if name.contains('\\')
        || name.chars().any(char::is_control)
        || Path::new(name)
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return false;
    }
    if name == "config.toml" {
        return true;
    }
    if let Some(name) = name.strip_prefix("data/") {
        if DATA_FILES.contains(&name) {
            return true;
        }
        return name.strip_prefix("dicts/").is_some_and(|file| {
            !file.contains('/')
                && !file.starts_with('.')
                && (file.ends_with(".qj") || file.ends_with(".tsv"))
        });
    }
    name.strip_prefix("themes/qingjian-custom/")
        .is_some_and(|file| !file.is_empty() && !file.split('/').any(|c| c.starts_with('.')))
}

fn sanitize(source: &str) -> Result<String, String> {
    let mut doc: toml_edit::DocumentMut = source
        .parse()
        .map_err(|_| "配置文件有语法错误，无法备份".to_owned())?;
    strip_secrets(doc.as_table_mut());
    if doc.get("predict").is_none() {
        doc["predict"] = toml_edit::table();
    }
    doc["predict"]["enabled"] = toml_edit::value(false);
    Ok(doc.to_string())
}
fn is_secret(key: &str) -> bool {
    [
        "api_key",
        "token",
        "password",
        "secret",
        "access_token",
        "refresh_token",
    ]
    .contains(&key.to_lowercase().as_str())
}
fn strip_value(value: &mut toml_edit::Value) {
    if let Some(table) = value.as_inline_table_mut() {
        let keys: Vec<_> = table.iter().map(|(key, _)| key.to_owned()).collect();
        for key in keys {
            if is_secret(&key) {
                table.remove(&key);
            } else if let Some(child) = table.get_mut(&key) {
                strip_value(child);
            }
        }
    } else if let Some(array) = value.as_array_mut() {
        for value in array.iter_mut() {
            strip_value(value);
        }
    }
}
fn strip_secrets(table: &mut toml_edit::Table) {
    let keys: Vec<_> = table.iter().map(|(key, _)| key.to_owned()).collect();
    for key in keys {
        if is_secret(&key) {
            table.remove(&key);
        } else if let Some(item) = table.get_mut(&key) {
            match item {
                toml_edit::Item::Table(child) => strip_secrets(child),
                toml_edit::Item::ArrayOfTables(array) => {
                    for child in array.iter_mut() {
                        strip_secrets(child);
                    }
                }
                toml_edit::Item::Value(value) => strip_value(value),
                toml_edit::Item::None => {}
            }
        }
    }
}
impl Archive {
    pub fn capture(paths: &BackupPaths) -> Result<Self, String> {
        let source = match std::fs::read_to_string(&paths.config) {
            Ok(source) => source,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                crate::config::TEMPLATE.to_owned()
            }
            Err(error) => return Err(error.to_string()),
        };
        let mut archive = Self {
            version: ARCHIVE_VERSION,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|e| e.to_string())?
                .as_secs(),
            files: BTreeMap::new(),
            checksums: BTreeMap::new(),
        };
        archive.files.insert(
            "config.toml".into(),
            BASE64_STANDARD.encode(sanitize(&source)?),
        );
        for name in DATA_FILES {
            archive.capture_file(&paths.data.join(name), &format!("data/{name}"))?;
        }
        archive.capture_tree(&paths.data.join("dicts"), "data/dicts")?;
        archive.capture_tree(
            &paths.themes.join("qingjian-custom"),
            "themes/qingjian-custom",
        )?;
        for (name, data) in &archive.files {
            archive.checksums.insert(
                name.clone(),
                checksum(&BASE64_STANDARD.decode(data).map_err(|_| "备份正文损坏")?),
            );
        }
        archive.validate()?;
        Ok(archive)
    }
    fn capture_file(&mut self, path: &Path, logical: &str) -> Result<(), String> {
        if !allowed(logical) {
            return Ok(());
        }
        match std::fs::symlink_metadata(path) {
            Ok(meta) if meta.is_file() => {
                let existing: usize = self.files.values().map(String::len).sum();
                if self.files.len() >= 10000
                    || existing as u64 + meta.len().saturating_mul(4).div_ceil(3)
                        > MAX_ARCHIVE_BYTES as u64
                {
                    return Err("备份超过归档大小或文件数上限".into());
                }
                if meta.len() > MAX_DATA_BYTES as u64 {
                    return Err("单个数据文件超过 64 MiB".into());
                }
                let data = std::fs::read(path).map_err(|e| e.to_string())?;
                self.files
                    .insert(logical.into(), BASE64_STANDARD.encode(data));
            }
            Ok(_) => return Err(format!("拒绝备份非普通文件：{logical}")),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.to_string()),
        }
        Ok(())
    }
    fn capture_tree(&mut self, dir: &Path, prefix: &str) -> Result<(), String> {
        if dir.is_symlink() {
            return Err(format!("拒绝备份符号链接：{prefix}"));
        }
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e.to_string()),
        };
        for entry in entries {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            let name = format!("{prefix}/{}", entry.file_name().to_string_lossy());
            if path.is_dir() {
                if prefix.starts_with("themes/") {
                    self.capture_tree(&path, &name)?;
                }
            } else {
                self.capture_file(&path, &name)?;
            }
        }
        Ok(())
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.version != ARCHIVE_VERSION {
            return Err("不支持的备份版本".into());
        }
        if self.files.len() > 10000 {
            return Err("备份文件数超过上限".into());
        }
        if self.checksums.len() != self.files.len() {
            return Err("备份校验清单不完整".into());
        }
        let mut total = 0usize;
        for (name, data) in &self.files {
            if !allowed(name) {
                return Err(format!("备份包含不允许的路径：{name}"));
            }
            if data.len() > MAX_ARCHIVE_BYTES {
                return Err("备份超过大小上限".into());
            }
            let decoded = BASE64_STANDARD
                .decode(data)
                .map_err(|_| "备份正文损坏".to_owned())?;
            if self.checksums.get(name) != Some(&checksum(&decoded)) {
                return Err(format!("备份文件校验失败：{name}"));
            }
            total = total.checked_add(decoded.len()).ok_or("备份大小溢出")?;
            if total > MAX_DATA_BYTES {
                return Err("备份用户数据超过 64 MiB".into());
            }
            if name == "config.toml" {
                let text =
                    std::str::from_utf8(&decoded).map_err(|_| "备份配置不是 UTF-8".to_owned())?;
                let config: Config = toml::from_str(text).map_err(|_| "备份配置损坏".to_owned())?;
                if config.predict.enabled
                    || config
                        .predict
                        .api_key
                        .as_deref()
                        .is_some_and(|k| !k.is_empty())
                {
                    return Err("备份云服务须关闭，且不能包含 API Key".into());
                }
                let doc: toml_edit::DocumentMut =
                    text.parse().map_err(|_| "备份配置损坏".to_owned())?;
                let mut stripped = doc.clone();
                strip_secrets(stripped.as_table_mut());
                if stripped.to_string() != doc.to_string() {
                    return Err("备份包含凭据字段".into());
                }
                if !crate::settings::validate(&config).is_empty() {
                    return Err("备份配置校验失败".into());
                }
            } else if name.starts_with("data/") && name.ends_with(".tsv") {
                std::str::from_utf8(&decoded).map_err(|_| "备份学习数据不是 UTF-8".to_owned())?;
            }
        }
        if !self.files.contains_key("config.toml") {
            return Err("备份缺少配置文件".into());
        }
        Ok(())
    }
    pub fn read(path: &Path) -> Result<Self, String> {
        let meta = std::fs::symlink_metadata(path).map_err(|e| e.to_string())?;
        if !meta.is_file() || meta.len() > MAX_ARCHIVE_BYTES as u64 {
            return Err("备份必须是小于 96 MiB 的普通文件".into());
        }
        let archive: Self =
            serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
                .map_err(|_| "备份格式损坏".to_owned())?;
        archive.validate()?;
        Ok(archive)
    }
    pub fn write(&self, path: &Path) -> Result<(), String> {
        self.validate()?;
        let data = serde_json::to_vec(self).map_err(|e| e.to_string())?;
        if data.len() > MAX_ARCHIVE_BYTES {
            return Err("备份归档超过 96 MiB".into());
        }
        credentials::write_private(path, &data).map_err(|e| e.to_string())
    }
}
#[cfg(test)]
mod tests;

fn checksum(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
