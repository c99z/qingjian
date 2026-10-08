//! 批量保存的失败原因。字段级冲突不是错误，见 [`crate::settings::ApplyOutcome::Conflicts`]。

use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ApplyError {
    #[error("failed to read config {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// 当前文件有语法错误：宁可什么都不写，也不能替用户「修复」成丢了注释的文件。
    #[error("config {path} is broken; refusing to overwrite it: {message}")]
    Broken { path: PathBuf, message: String },

    #[error("invalid custom phrases: {0}")]
    Phrases(String),

    #[error("failed to write config {path}: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}
