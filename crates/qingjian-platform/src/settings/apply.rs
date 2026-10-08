//! 批量保存：草稿（[`ConfigPatch`]）相对快照（[`SettingsSnapshot`]）与当前文件逐字段三路比较，
//! 无冲突时一次性原子写入；保留未知字段、注释与原有顺序。
//!
//! 三路比较（base = 快照值 / current = 当前文件值 / draft = 草稿新值）：
//! - 草稿没改的字段：不碰，外部改了就跟着外部走（自动跟随外部更新）。
//! - 草稿改了、外部没改：写入草稿值。
//! - 草稿改了、外部也改了且两边不一样：进冲突列表，整体拒绝写入。
//! - 草稿与外部改成了同一个值：收敛，不用写也不算冲突。

use std::path::Path;

use toml_edit::DocumentMut;

use crate::config::{Config, TEMPLATE};

use super::conflict::FieldConflict;
use super::error::ApplyError;
use super::outcome::ApplyOutcome;
use super::patch::{ConfigPatch, put_value};
use super::snapshot::SettingsSnapshot;

/// 把草稿相对 `base` 快照批量写入 `path`。文件不存在时从模板起步；当前文件有语法错误时
/// 拒绝写入（[`ApplyError::Broken`]）——不能替用户「修复」成丢了注释的文件。
pub fn apply(
    path: &Path,
    base: &SettingsSnapshot,
    patch: &ConfigPatch,
) -> Result<ApplyOutcome, ApplyError> {
    let source = match std::fs::read_to_string(path) {
        Ok(source) => source,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => TEMPLATE.to_owned(),
        Err(source) => {
            return Err(ApplyError::Read {
                path: path.to_owned(),
                source,
            });
        }
    };
    let mut document: DocumentMut =
        source
            .parse::<DocumentMut>()
            .map_err(|error| ApplyError::Broken {
                path: path.to_owned(),
                message: error.to_string(),
            })?;
    // 快照原文损坏时所有字段的 base 都按「不存在」算：草稿改过的字段只要文件里有值就算冲突
    let base_document: Option<DocumentMut> = base.source.as_deref().and_then(|s| s.parse().ok());

    let mut writes = Vec::new();
    let mut conflicts = Vec::new();
    for edit in &patch.edits {
        let base_value = base_document
            .as_ref()
            .and_then(|doc| value_repr(doc, &edit.section, &edit.key));
        let current_value = value_repr(&document, &edit.section, &edit.key);
        let draft_value = edit.value.to_string();
        if Some(&draft_value) == base_value.as_ref() {
            continue;
        }
        if current_value == base_value {
            writes.push(edit);
        } else if current_value.as_ref() == Some(&draft_value) {
            // 外部已改成同一值，收敛
        } else {
            conflicts.push(FieldConflict {
                field: edit.field(),
                base: base_value,
                current: current_value,
                draft: draft_value,
            });
        }
    }

    let mut new_phrases = None;
    if let Some(phrases) = &patch.custom_phrases {
        qingjian_core::custom_phrase::validate_phrases(phrases).map_err(ApplyError::Phrases)?;
        if *phrases != base.config.custom_phrases {
            // 文档能解析不代表 Config 能（短语位置冲突在 Config 这一层才报），读不出来按冲突处理
            match toml::from_str::<Config>(&source) {
                Ok(current) if current.custom_phrases == *phrases => {}
                Ok(current) if current.custom_phrases == base.config.custom_phrases => {
                    new_phrases = Some(phrases.clone());
                }
                current => conflicts.push(FieldConflict {
                    field: "custom_phrases".to_owned(),
                    base: Some(describe_phrases(&base.config.custom_phrases)),
                    current: current.ok().map(|c| describe_phrases(&c.custom_phrases)),
                    draft: describe_phrases(phrases),
                }),
            }
        }
    }

    if !conflicts.is_empty() {
        return Ok(ApplyOutcome::Conflicts(conflicts));
    }
    if writes.is_empty() && new_phrases.is_none() {
        return Ok(ApplyOutcome::Applied);
    }
    for edit in writes {
        put_value(&mut document, &edit.section, &edit.key, &edit.value);
    }
    if let Some(phrases) = new_phrases {
        crate::config::custom_phrases::update_custom_phrases(&mut document, &phrases);
    }
    write_atomic(path, &document.to_string())?;
    Ok(ApplyOutcome::Applied)
}

/// 字段值的 TOML 写法（不含排版），用于三路比较；字段不存在为 `None`。
/// toml_edit 的 `Value` Display 会带上前后空白 decor，比较前必须清掉。
fn value_repr(document: &DocumentMut, section: &str, key: &str) -> Option<String> {
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

fn describe_phrases(phrases: &[qingjian_core::CustomPhrase]) -> String {
    format!("{} 条短语", phrases.len())
}

/// 原子写配置文件；目录还没有就先建（新账户第一次打开设置时输入法可能还没跑过）。
fn write_atomic(path: &Path, text: &str) -> Result<(), ApplyError> {
    let write = || {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        qingjian_core::storage::write_atomic_str(path, text)
    };
    write().map_err(|source| ApplyError::Write {
        path: path.to_owned(),
        source,
    })
}
