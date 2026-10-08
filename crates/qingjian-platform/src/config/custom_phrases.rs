//! 自定义短语的文档内更新：在已解析的 TOML 文档里整体替换 `[[custom_phrases]]`，
//! 重排或删除时注释跟随原规则。`Config::set_custom_phrases`（单键读写）与
//! `settings` 模块的批量保存共用这一份逻辑。

use toml_edit::DocumentMut;

/// 把文档里的 `[[custom_phrases]]` 整体替换成 `phrases`：先按输入码和位置匹配旧表（注释跟随原规则），
/// 等长列表中改了输入码或位置的条目沿用其未被占用的原表。
pub(crate) fn update_custom_phrases(
    document: &mut DocumentMut,
    phrases: &[qingjian_core::CustomPhrase],
) {
    let old = document
        .get("custom_phrases")
        .and_then(toml_edit::Item::as_array_of_tables)
        .cloned()
        .unwrap_or_default();
    let mut used = std::collections::BTreeSet::new();
    // 先按输入码和位置匹配，重排或删除时注释跟随原规则。
    let mut matches: Vec<_> = phrases
        .iter()
        .map(|p| {
            let found = old
                .iter()
                .enumerate()
                .find(|(_, t)| {
                    t.get("code").and_then(toml_edit::Item::as_str) == Some(p.code.as_str())
                        && t.get("position").and_then(toml_edit::Item::as_integer)
                            == Some(p.position as i64)
                })
                .map(|(i, _)| i);
            if let Some(i) = found {
                used.insert(i);
            }
            found
        })
        .collect();
    // 等长列表中修改了输入码或位置的条目，沿用其未被占用的原表。
    if old.len() == phrases.len() {
        for (i, matched) in matches.iter_mut().enumerate() {
            if matched.is_none() && used.insert(i) {
                *matched = Some(i);
            }
        }
    }
    let mut tables = toml_edit::ArrayOfTables::new();
    for (p, matched) in phrases.iter().zip(matches) {
        let mut t = matched
            .and_then(|i| old.get(i))
            .cloned()
            .unwrap_or_default();
        for (key, mut value) in [
            ("code", toml_edit::Value::from(p.code.as_str())),
            ("text", toml_edit::Value::from(p.text.as_str())),
            ("position", toml_edit::Value::from(p.position as i64)),
            ("enabled", toml_edit::Value::from(p.enabled)),
        ] {
            if let Some(previous) = t.get(key).and_then(toml_edit::Item::as_value) {
                if previous.to_string() == value.to_string() {
                    continue;
                }
                *value.decor_mut() = previous.decor().clone();
            }
            t[key] = toml_edit::Item::Value(value);
        }
        // 序列化按文档位置排序；统一锚点后，同组条目使用本次列表顺序。
        t.set_position(old.iter().filter_map(toml_edit::Table::position).min());
        tables.push(t);
    }
    document["custom_phrases"] = toml_edit::Item::ArrayOfTables(tables);
}
