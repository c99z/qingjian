//! 上传的前置版本：过期上传不能静默覆盖其他设备的新快照。
use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UploadRequest {
    pub expected_revision: u64,

    pub archive: crate::backup::Archive,
}
