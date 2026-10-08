//! 用户数据备份传输，只通过固定传输目录提交给本机服务。
use crate::adapters::admin::AdminClient;
use qingjian_platform::backup::Archive;
use qingjian_platform::settings::ComponentStatus;
use std::path::Path;

pub fn restore(admin: &AdminClient, archive: &Archive) -> Result<(), String> {
    archive.validate()?;
    let id = uuid::Uuid::new_v4().to_string();
    let dir = qingjian_platform::dirs::linux::user_dir().join("transfers");
    if dir.is_symlink() {
        return Err("传输目录不能是符号链接".into());
    }
    let path = dir.join(format!("{id}.qjb"));
    archive.write(&path)?;
    let result = match admin.restore(id) {
        Ok(ComponentStatus::Applied) => Ok(()),
        Ok(ComponentStatus::Failed(message)) => Err(message),
        Ok(_) => Err("服务未确认恢复".into()),
        Err(error) => Err(error.user_message()),
    };
    let _ = std::fs::remove_file(path);
    result
}
pub fn export(admin: &AdminClient, target: &Path) -> Result<(), String> {
    let path = admin.backup().map_err(|e| e.user_message())?;
    Archive::read(&path)?.write(target)
}
