//! ThemePatch 的定义与实现。

/// 主题里与释义（候选注记）相关的三个字段；`None` = 跟随主题里的写法，不改。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ThemePatch {
    /// `CandidateCommentColor`：普通候选的释义颜色。
    pub comment_color: Option<String>,

    /// `HighlightCandidateCommentColor`：高亮候选的释义颜色。
    pub highlight_comment_color: Option<String>,

    /// `CommentTextSizeFactor`：释义字号相对候选字号的百分比（Fcitx5 缺省 75）。
    pub comment_size_factor: Option<u32>,

    /// 普通候选中文颜色（NormalColor）。
    pub normal_color: Option<String>,

    /// 高亮候选中文颜色（HighlightCandidateColor）。
    pub highlight_color_text: Option<String>,
}
