//! ClassicUiPanel 的定义与实现。

/// 经典面板是否接管候选显示。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassicUiPanel {
    /// 已启用；其他面板接管时须提示支持范围。
    Active,

    /// 装了但被禁用。
    Disabled,

    ///  addon 列表里根本没有。
    Missing,
}
