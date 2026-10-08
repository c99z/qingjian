//! 候选预览的展示参数。

use crate::adapters::fcitx5::FontDesc;
use qingjian_platform::LayoutMode;

/// 一次重画需要的全部取值（都取「草稿优先、现状兜底」的生效值）。
#[derive(Debug, Clone)]
pub struct PreviewModel {
    pub layout: LayoutMode,

    pub font: FontDesc,

    /// 普通 / 高亮释义颜色；`None` 用示意缺省（主题色预览不出来）。
    pub comment_color: Option<String>,

    pub highlight_comment_color: Option<String>,

    /// 释义字号相对候选的百分比。
    pub comment_factor: u32,

    pub normal_color: String,

    pub highlight_color_text: String,
}
