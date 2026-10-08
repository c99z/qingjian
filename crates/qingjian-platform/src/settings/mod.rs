//! 共享设置基础：配置快照、草稿补丁、保存前校验、批量保存（三路比较 + 冲突检测）与生效报告类型。
//!
//! 配置文件仍是唯一事实源；这里管「编辑到落盘」这一段——读快照、攒草稿、校验、无冲突才原子写入，
//! 生效与重载由各壳的服务负责（Linux 设置计划的 P1 阶段）。

mod apply;
mod conflict;
mod error;
mod field_error;
mod file_version;
mod outcome;
mod patch;
mod report;
mod resources;
mod snapshot;
mod validate;

pub use apply::apply;
pub use conflict::FieldConflict;
pub use error::ApplyError;
pub use field_error::FieldError;
pub use file_version::FileVersion;
pub use outcome::ApplyOutcome;
pub use patch::{ConfigPatch, FieldEdit};
pub use report::{ApplyReport, ComponentStatus};
pub use resources::{NamedFile, ResourceCapabilities};
pub use snapshot::SettingsSnapshot;
pub use validate::validate;
