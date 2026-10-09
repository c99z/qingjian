//! 运行中的一致性备份：`VACUUM INTO` 读的是一个事务快照，服务不用停。
use crate::{error::ApiError, store::AccountStore};
use std::path::Path;

impl AccountStore {
    /// 写到 `dest`；目标已存在时拒绝，不覆盖旧备份。
    pub fn backup_to(&self, dest: &Path) -> Result<(), ApiError> {
        if dest.exists() {
            return Err(ApiError::Invalid(format!(
                "备份目标已存在：{}",
                dest.display()
            )));
        }
        let dest = dest
            .to_str()
            .ok_or_else(|| ApiError::Invalid("备份路径须为 UTF-8".into()))?;
        let db = self.db.lock().map_err(|_| ApiError::Internal)?;
        db.execute("VACUUM INTO ?1", [dest])?;
        Ok(())
    }
}
