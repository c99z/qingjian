//! PanelProbe 的定义与实现。

use crate::adapters::fcitx5::probe::ClassicUiPanel;

/// 面板检测结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanelProbe {
    pub classic_ui: ClassicUiPanel,

    /// 其他启用中的界面插件名（如 kimpanel）：桌面若由它们显示候选，经典面板设置不生效。
    pub other_panels: Vec<String>,
}
