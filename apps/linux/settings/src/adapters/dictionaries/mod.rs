//! 词库列表、验证后原子导入和仅用户目录的移除。
mod info;
pub use info::DictionaryInfo;
use qingjian_dictionary::Dictionary;
use qingjian_platform::extra_dictionaries;
use std::path::Path;

pub fn list(root: Option<&Path>, user: &Path) -> Vec<DictionaryInfo> {
    let bundled = root
        .map(|r| extra_dictionaries::list(&r.join("data/generated/dicts")))
        .unwrap_or_default();
    bundled
        .into_iter()
        .map(|(s, p)| (s, p, true))
        .chain(
            extra_dictionaries::list(&user.join("dicts"))
                .into_iter()
                .map(|(s, p)| (s, p, false)),
        )
        .map(|(stem, path, bundled)| {
            let description = match Dictionary::from_path(&path) {
                Ok(dict) => format!(
                    "{} · {} 词条 · {}",
                    if bundled {
                        "随包词库"
                    } else {
                        "用户词库"
                    },
                    dict.len(),
                    dict.metadata().map_or("本机文件", |m| m.source.as_str())
                ),
                Err(error) => format!("无法加载：{error}"),
            };
            DictionaryInfo {
                stem,
                path,
                bundled,
                description,
            }
        })
        .collect()
}
pub fn validate_name(name: &str) -> Result<(), String> {
    if name.is_empty()
        || name.len() > 120
        || name.starts_with('.')
        || name
            .chars()
            .any(|c| c.is_control() || matches!(c, '/' | '\\' | ':'))
    {
        return Err("词库名称不能为空，也不能包含路径分隔符".into());
    }
    Ok(())
}
pub fn exists(user: &Path, name: &str) -> bool {
    ["qj", "tsv"]
        .iter()
        .any(|ext| user.join("dicts").join(format!("{name}.{ext}")).exists())
}

pub fn import(source: &Path, user: &Path, name: &str, replace: bool) -> Result<String, String> {
    validate_name(name)?;
    let dir = user.join("dicts");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let stage = tempfile::tempdir_in(&dir).map_err(|e| e.to_string())?;
    let imported = qingjian_dictionary::import::import(source, stage.path())
        .map_err(|e| format!("导入失败：{e}"))?;
    let target = dir.join(format!("{name}.qj"));
    if !replace && exists(user, name) {
        return Err("同名词库已存在，请重命名或确认替换".into());
    }
    if target.is_symlink() {
        return Err("目标词库是符号链接，未替换".into());
    }
    // 在同一目录准备完整文件；坏输入不会触碰原文件。
    std::fs::rename(&imported.path, &target).map_err(|e| e.to_string())?;
    Ok(format!("已导入 {name}，{} 词条", imported.entries))
}
pub fn remove(user: &Path, name: &str) -> Result<(), String> {
    validate_name(name)?;
    for ext in ["qj", "tsv"] {
        let path = user.join("dicts").join(format!("{name}.{ext}"));
        if path.is_symlink() {
            return Err("词库是符号链接，未移除".into());
        }
        match std::fs::remove_file(path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests;
