//! CommentStyle 的定义与实现。

/// 当前主题的释义样式（从它的 `theme.conf` 读出来做草稿基准）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CommentStyle {
    pub color: Option<String>,

    pub highlight_color: Option<String>,

    pub size_factor: Option<u32>,

    /// 普通候选中文颜色（NormalColor）。
    pub normal_color: Option<String>,

    /// 高亮候选中文颜色（HighlightCandidateColor）。
    pub highlight_color_text: Option<String>,
}
