//! AppearanceValues 的定义与实现。

use crate::adapters::fcitx5::font::FontDesc;

/// 一次外观草稿里用户想改的东西；`None` = 这一项不动。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AppearanceValues {
    /// classicui `Font`。
    pub font: Option<FontDesc>,

    /// classicui `Theme`（主题名）。
    pub theme: Option<String>,

    /// 普通候选的释义颜色（主题字段，改了就要走定制主题）。
    pub comment_color: Option<String>,

    /// 高亮候选的释义颜色。
    pub highlight_comment_color: Option<String>,

    /// 释义字号相对候选字号的百分比。
    pub comment_size_factor: Option<u32>,

    /// 普通候选中文颜色（NormalColor）。
    pub normal_color: Option<String>,

    /// 高亮候选中文颜色（HighlightCandidateColor）。
    pub highlight_color_text: Option<String>,
}
