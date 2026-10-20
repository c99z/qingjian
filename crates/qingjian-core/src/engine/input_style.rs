//! 中文输入时的候选策略；不改变中 / 英切换与用户主动调用的直输、短语、翻译。

use serde::{Deserialize, Serialize};

use super::Engine;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChineseInputStyle {
    /// 沿用旧配置的自动中英混输。
    #[default]
    Mixed,

    /// 专注拼音和中文纠错，不自动补英文或拼接英文片段。
    Chinese,
}

impl ChineseInputStyle {
    pub const ALL: [Self; 2] = [Self::Chinese, Self::Mixed];

    pub fn key(self) -> &'static str {
        match self {
            Self::Chinese => "chinese",
            Self::Mixed => "mixed",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Chinese => "专注中文",
            Self::Mixed => "中英混输",
        }
    }

    pub fn mixed(self) -> bool {
        self == Self::Mixed
    }

    /// 模型结果在展示时过滤，因此切换后晚到的结果与缓存也遵守当前策略。
    pub(super) fn allows_generated(self, text: &str) -> bool {
        self.mixed()
            || !text
                .chars()
                .any(|c| c.is_ascii_alphabetic() || matches!(c, 'Ａ'..='Ｚ' | 'ａ'..='ｚ'))
    }
}

impl Engine {
    pub fn set_chinese_input_style(&mut self, style: ChineseInputStyle) {
        if self.chinese_input_style != style {
            self.chinese_input_style = style;
            // 英文词是否阻挡拼音纠错取决于策略，旧结论不能沿用。
            *self.correction_cache.borrow_mut() = None;
        }
    }

    pub fn chinese_input_style(&self) -> ChineseInputStyle {
        self.chinese_input_style
    }
}
