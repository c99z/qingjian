//! 本机凭据文件的解析、0600 原子写入与显式解析；不修改进程环境。
use qingjian_predict::PredictConfig;
use std::path::Path;

/// 显式配置 > 同目录 .env > 环境变量。
pub fn resolve(config: &PredictConfig, config_path: &Path) -> Result<PredictConfig, String> {
    let mut result = config.clone();
    if result
        .api_key
        .as_deref()
        .is_some_and(|key| !key.trim().is_empty())
    {
        return Ok(result);
    }
    let path = config_path.with_file_name(".env");
    result.api_key = read_key(&path, &config.api_key_env)?;
    Ok(result)
}

pub fn read_key(path: &Path, name: &str) -> Result<Option<String>, String> {
    if !name
        .bytes()
        .enumerate()
        .all(|(i, c)| c == b'_' || c.is_ascii_alphabetic() || (i > 0 && c.is_ascii_digit()))
        || name.is_empty()
    {
        return Err("密钥环境变量名不合法".into());
    }
    match dotenvy::from_path_iter(path) {
        Ok(entries) => {
            let mut key = None;
            for entry in entries {
                let (entry_name, value) = entry.map_err(|_| "本机凭据文件格式有误".to_owned())?;
                if entry_name == name && !value.trim().is_empty() {
                    key = Some(value);
                }
            }
            Ok(key)
        }
        Err(dotenvy::Error::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err("无法读取本机凭据文件".into()),
    }
}

/// 保留其他凭据项；值用单引号避免环境插值，API key 不接受换行或引号。
pub fn write_key(path: &Path, name: &str, key: &str) -> Result<(), String> {
    read_key(path, name)?;
    if key.chars().any(|c| c.is_control() || c == '\'') {
        return Err("API Key 含不支持的字符".into());
    }
    let previous = match std::fs::read_to_string(path) {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(_) => return Err("无法读取本机凭据文件".into()),
    };
    let mut lines: Vec<&str> = previous
        .lines()
        .filter(|line| {
            let line = line
                .trim_start()
                .strip_prefix("export ")
                .unwrap_or(line.trim_start());
            line.split_once('=')
                .is_none_or(|(field, _)| field.trim() != name)
        })
        .collect();
    let entry = format!("{name}='{key}'");
    lines.push(&entry);
    write_private(path, format!("{}\n", lines.join("\n")).as_bytes())
        .map_err(|_| "无法保存本机凭据文件".into())
}

pub fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let parent = path
        .parent()
        .ok_or_else(|| std::io::Error::other("missing parent"))?;
    std::fs::create_dir_all(parent)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        temporary
            .as_file()
            .set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }
    temporary.write_all(bytes)?;
    temporary.as_file().sync_all()?;
    temporary.persist(path).map_err(|error| error.error)?;
    Ok(())
}

#[cfg(test)]
mod tests;
