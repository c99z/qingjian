//! Fcitx5 适配层的出错类型；`NotRunning` 单列：Fcitx5 没跑不是失败，
//! 外观修改保留在草稿中，其余青简设置照常保存。

use std::path::PathBuf;

use gio::glib;

use super::appearance::FieldMismatch;

#[derive(Debug, thiserror::Error)]
pub enum Fcitx5Error {
    /// 会话总线上没有 `org.fcitx.Fcitx5`。
    #[error("fcitx5 is not running")]
    NotRunning,

    #[error("dbus: {0}")]
    Bus(#[from] glib::Error),

    #[error("invalid appearance: {0}")]
    Invalid(String),

    /// 回包结构与预期不符（Fcitx5 版本不配套）。
    #[error("unexpected reply: {0}")]
    BadReply(String),

    /// 要复制的源主题在两个主题目录里都没有。
    #[error("theme not found: {0}")]
    ThemeNotFound(String),

    #[error("theme file {}: {source}", path.display())]
    ThemeIo {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("appearance changed externally")]
    Conflict(Vec<FieldMismatch>),

    #[error("classic panel is unavailable")]
    PanelUnavailable,

    /// 写入后读回比对不一致：附各字段的实际值。
    #[error("read-back mismatch")]
    Verify(Vec<FieldMismatch>),
}

impl Fcitx5Error {
    /// 给用户看的中文说明。
    pub fn user_message(&self) -> String {
        match self {
            Self::Invalid(message) => message.clone(),
            Self::NotRunning => "Fcitx5 未运行，外观修改未应用，仍保留在草稿中".to_owned(),
            Self::Bus(error) => format!("与 Fcitx5 通信失败：{error}"),
            Self::BadReply(detail) => {
                format!("Fcitx5 的回应无法识别（{detail}），版本可能不配套")
            }
            Self::ThemeNotFound(name) => format!("找不到主题「{name}」的目录，无法复制"),
            Self::ThemeIo { path, source } => {
                format!("写入定制主题失败（{}）：{source}", path.display())
            }
            Self::PanelUnavailable => "经典面板未启用，外观修改保留在草稿中".to_owned(),
            Self::Conflict(fields) => format!(
                "外观被其他程序修改（{}），请重新载入后再应用；草稿已保留",
                fields
                    .iter()
                    .map(|f| f.label)
                    .collect::<Vec<_>>()
                    .join("、")
            ),
            Self::Verify(mismatches) => {
                let detail: Vec<String> = mismatches
                    .iter()
                    .map(|m| format!("{}实际为「{}」", m.label, m.actual))
                    .collect();
                format!(
                    "外观写入后与 Fcitx5 的实际值不一致（{}）",
                    detail.join("，")
                )
            }
        }
    }
}
