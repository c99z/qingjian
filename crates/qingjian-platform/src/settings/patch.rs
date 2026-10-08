//! 配置补丁：设置界面编辑期间累积的草稿——一组字段编辑，外加自定义短语的整体替换。
//! 补丁不直接落盘：先 [`ConfigPatch::draft`] 出完整草稿配置给 [`crate::settings::validate`] 校验，
//! 再交给 [`crate::settings::apply`] 与当前文件做三路比较后批量写入。

use toml_edit::{DocumentMut, Value};

use crate::config::{Config, TEMPLATE};

use super::snapshot::SettingsSnapshot;

/// 一个字段的编辑：`[section] key = value`。
#[derive(Debug, Clone)]
pub struct FieldEdit {
    pub section: String,

    pub key: String,

    pub value: Value,
}

impl FieldEdit {
    pub fn new(section: &str, key: &str, value: impl Into<Value>) -> Self {
        Self {
            section: section.to_owned(),
            key: key.to_owned(),
            value: value.into(),
        }
    }

    /// 字段路径，如 `general.page_size`。
    pub fn field(&self) -> String {
        format!("{}.{}", self.section, self.key)
    }
}

/// 一份草稿的全部修改。
#[derive(Debug, Clone, Default)]
pub struct ConfigPatch {
    /// 标量与数组字段编辑；同字段多次设置以最后一次为准（写文档时后者覆盖前者）。
    pub edits: Vec<FieldEdit>,

    /// 自定义短语整体替换；`None` 不动这一节。
    pub custom_phrases: Option<Vec<qingjian_core::CustomPhrase>>,
}

impl ConfigPatch {
    pub fn new() -> Self {
        Self::default()
    }

    /// 追加一个字段编辑。
    pub fn set(&mut self, section: &str, key: &str, value: impl Into<Value>) -> &mut Self {
        self.edits.push(FieldEdit::new(section, key, value));
        self
    }

    /// 追加一个字符串数组字段编辑（如 `[dictionaries] domains`）。
    pub fn set_array<S: AsRef<str>>(
        &mut self,
        section: &str,
        key: &str,
        values: &[S],
    ) -> &mut Self {
        let mut array = toml_edit::Array::new();
        for value in values {
            array.push(value.as_ref());
        }
        self.set(section, key, array)
    }

    /// 整体替换自定义短语。
    pub fn set_custom_phrases(&mut self, phrases: Vec<qingjian_core::CustomPhrase>) -> &mut Self {
        self.custom_phrases = Some(phrases);
        self
    }

    pub fn is_empty(&self) -> bool {
        self.edits.is_empty() && self.custom_phrases.is_none()
    }

    /// 把补丁套到快照上得到完整草稿配置（校验用）。快照原文损坏或补丁类型对不上字段时报错。
    pub fn draft(&self, base: &SettingsSnapshot) -> Result<Config, String> {
        let source = base.source.clone().unwrap_or_else(|| TEMPLATE.to_owned());
        let mut document: DocumentMut = source.parse::<DocumentMut>().map_err(|e| e.to_string())?;
        self.apply_to(&mut document);
        toml::from_str::<Config>(&document.to_string()).map_err(|e| e.to_string())
    }

    /// 把补丁的全部编辑写进已解析的文档（不重排、不动其他内容）。
    pub(crate) fn apply_to(&self, document: &mut DocumentMut) {
        for edit in &self.edits {
            put_value(document, &edit.section, &edit.key, &edit.value);
        }
        if let Some(phrases) = &self.custom_phrases {
            crate::config::custom_phrases::update_custom_phrases(document, phrases);
        }
    }
}

/// 原地写一个键：分节不存在先建成标准表；旧值的排版（含行尾注释）跟随到新值。
pub(crate) fn put_value(document: &mut DocumentMut, section: &str, key: &str, value: &Value) {
    if !document.get(section).is_some_and(toml_edit::Item::is_table) {
        document[section] = toml_edit::table();
    }
    let mut value = value.clone();
    if let Some(previous) = document
        .get(section)
        .and_then(|item| item.get(key))
        .and_then(toml_edit::Item::as_value)
    {
        *value.decor_mut() = previous.decor().clone();
    }
    document[section][key] = toml_edit::Item::Value(value);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draft_applies_edits_onto_the_base_source() {
        let dir = std::env::temp_dir().join(format!("qingjian-patch-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        std::fs::write(&path, "[general]\npage_size = 5\n").unwrap();
        let snapshot = SettingsSnapshot::load(&path).unwrap();
        let mut patch = ConfigPatch::new();
        patch.set("predict", "enabled", true);
        let draft = patch.draft(&snapshot).unwrap();
        assert!(draft.predict.enabled);
        assert_eq!(draft.general.page_size, 5);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn draft_starts_from_the_template_when_the_file_is_missing() {
        let path = std::env::temp_dir().join("qingjian-patch-missing/config.toml");
        let snapshot = SettingsSnapshot::load(&path).unwrap();
        let draft = ConfigPatch::new().draft(&snapshot).unwrap();
        assert_eq!(draft, Config::default());
    }
}
