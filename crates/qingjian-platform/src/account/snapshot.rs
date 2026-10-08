//! 下载的账号数据快照。
use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize)]
pub struct RemoteSnapshot {
    pub revision: u64,

    pub archive: crate::backup::Archive,
}
