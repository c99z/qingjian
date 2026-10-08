//! 后台探测的结果，返回 GTK 主线程后更新控件。

use crate::adapters::fcitx5::{ClassicUiState, PanelProbe};

/// Fcitx5 探测结果（后台线程产出，回主线程更新）。
pub(super) enum ProbeOutcome {
    Running {
        panels: PanelProbe,
        state: Option<ClassicUiState>,
    },

    NotRunning,

    Failed(String),
}
