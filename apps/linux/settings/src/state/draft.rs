//! 草稿存储：一份配置快照（基准）加一组草稿修改（[`ConfigPatch`]）。
//! 控件编辑只往这里喂字段改动；脏标记、改回原值自动撤销、重新载入与保存后的基准推进都在这一层，不碰 GUI。

use std::path::Path;

use qingjian_platform::settings::{ConfigPatch, FieldEdit, SettingsSnapshot};
use qingjian_platform::{Config, ConfigError};
use toml_edit::{DocumentMut, Value};

use super::appearance::AppearanceDraft;
use super::credential::{CredentialEdit, read_source};

pub struct DraftStore {
    snapshot: SettingsSnapshot,

    patch: ConfigPatch,

    /// Fcitx5 外观草稿：不进 `config.toml`，保存时走 D-Bus 单独应用、单独报告。
    appearance: AppearanceDraft,

    /// 仅待保存的密钥；不进入普通配置补丁。
    credential: Option<CredentialEdit>,

    credential_base: Result<Option<String>, String>,
}

impl DraftStore {
    /// 读配置文件建初始草稿（无修改）。
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        Ok(Self::new(SettingsSnapshot::load(path)?))
    }

    pub fn new(snapshot: SettingsSnapshot) -> Self {
        let credential_base = read_source(&snapshot.path.with_file_name(".env"));
        Self {
            snapshot,
            patch: ConfigPatch::new(),
            appearance: AppearanceDraft::new(),
            credential: None,
            credential_base,
        }
    }

    pub fn path(&self) -> &Path {
        &self.snapshot.path
    }

    pub fn snapshot(&self) -> &SettingsSnapshot {
        &self.snapshot
    }

    /// 当前生效配置（快照值）；控件初始与「撤销 / 重新载入」后的回填都读它。
    pub fn config(&self) -> &Config {
        &self.snapshot.config
    }

    pub fn patch(&self) -> &ConfigPatch {
        &self.patch
    }

    pub fn appearance(&self) -> &AppearanceDraft {
        &self.appearance
    }

    pub fn appearance_mut(&mut self) -> &mut AppearanceDraft {
        &mut self.appearance
    }

    /// 有没有未保存的修改（配置或外观任一）。
    pub fn is_dirty(&self) -> bool {
        !self.patch.is_empty() || self.appearance.is_dirty() || self.credential.is_some()
    }

    /// 记一处字段修改。同一字段只留最后一次；改回快照里的原值时移除这条编辑，
    /// 让脏标记如实反映「与文件内容有没有差别」。
    pub fn set(&mut self, section: &str, key: &str, value: impl Into<Value>) {
        let mut value = value.into();
        value.decor_mut().clear();
        self.patch
            .edits
            .retain(|edit| !(edit.section == section && edit.key == key));
        if base_value(&self.snapshot, section, key) != Some(value.to_string()) {
            self.patch.edits.push(FieldEdit::new(section, key, value));
        }
    }

    pub fn set_credential(&mut self, key: String) {
        self.credential = Some(CredentialEdit {
            key,
            base: self.credential_base.clone(),
        });
        // 清除旧显式密钥，让本机 .env 接管优先级。
        self.set("predict", "api_key", "");
    }

    pub fn credential(&self) -> Option<&CredentialEdit> {
        self.credential.as_ref()
    }

    pub fn cancel_credential(&mut self) {
        self.credential = None;
        self.patch
            .edits
            .retain(|edit| !(edit.section == "predict" && edit.key == "api_key"));
    }

    pub fn set_phrases(&mut self, phrases: Vec<qingjian_core::CustomPhrase>) {
        self.patch.custom_phrases = (phrases != self.config().custom_phrases).then_some(phrases);
    }

    /// 撤销全部未保存的修改，回到快照（含外观草稿）。
    pub fn revert(&mut self) {
        self.patch = ConfigPatch::new();
        self.credential = None;
        self.appearance.clear();
    }

    /// 重新读配置文件：放弃草稿，以最新文件内容为基准（冲突对话框里的「重新载入」）。
    pub fn reload(&mut self) -> Result<(), ConfigError> {
        let path = self.snapshot.path.clone();
        self.snapshot = SettingsSnapshot::load(&path)?;
        self.credential_base = read_source(&path.with_file_name(".env"));
        self.patch = ConfigPatch::new();
        self.credential = None;
        self.appearance.clear();
        Ok(())
    }

    /// 保存成功后推进基准：重新读快照、清空配置草稿。
    /// `appearance_applied` 为假时（Fcitx5 没跑或应用失败）外观草稿保留，等用户重试。
    pub fn mark_saved(&mut self, appearance_applied: bool) -> Result<(), ConfigError> {
        let appearance = std::mem::take(&mut self.appearance);
        self.reload()?;
        self.appearance = appearance;
        if appearance_applied {
            self.appearance.clear();
        }
        Ok(())
    }

    /// 草稿套到快照上的完整配置（校验用）。
    pub fn draft(&self) -> Result<Config, String> {
        self.patch.draft(&self.snapshot)
    }
}

/// 快照里某字段的 TOML 写法（清掉排版）；快照原文损坏或字段不存在为 `None`。
/// 与 `settings::apply` 的三路比较同口径，保证「改回原值就不算脏」与落盘时的跳过逻辑一致。
fn base_value(snapshot: &SettingsSnapshot, section: &str, key: &str) -> Option<String> {
    let document: DocumentMut = snapshot.source.as_deref()?.parse().ok()?;
    let item = document.get(section)?.get(key)?;
    Some(match item.as_value() {
        Some(value) => {
            let mut value = value.clone();
            value.decor_mut().clear();
            value.to_string()
        }
        None => item.to_string().trim().to_owned(),
    })
}

#[cfg(test)]
mod tests;
