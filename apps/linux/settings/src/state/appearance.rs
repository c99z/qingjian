//! Fcitx5 外观草稿：与青简配置草稿并列（不进 `config.toml`，应用时走 D-Bus 写 classicui）。
//! 基准来自适配层读出的经典面板当前配置；「改回现状就不算脏」、Fcitx5 没跑时草稿保留都在这层。

use crate::adapters::fcitx5::{AppearanceValues, ClassicUiState, FontDesc};

/// 外观草稿：各字段 `None` = 不动这一项（主题保持当前、释义样式跟随主题）。
#[derive(Debug, Clone, Default)]
pub struct AppearanceDraft {
    /// Fcitx5 探测出的当前配置；`None` = Fcitx5 没跑或探测失败（草稿留着，保存时如实报失败）。
    base: Option<ClassicUiState>,

    font: Option<FontDesc>,

    theme: Option<String>,

    normal_color: Option<String>,

    highlight_color_text: Option<String>,

    comment_color: Option<String>,

    highlight_comment_color: Option<String>,

    comment_size_factor: Option<u32>,
}

impl AppearanceDraft {
    pub fn new() -> Self {
        Self::default()
    }

    /// 探测结果进来：换基准。草稿不动——与新的现状一致的草稿自然不再算脏。
    pub fn set_base(&mut self, base: Option<ClassicUiState>) {
        if let (Some(old), Some(new)) = (&self.base, &mut base.clone()) {
            let mut merged = new.clone();
            if self.font.is_some() {
                merged.font = old.font.clone();
            }
            if self.theme.is_some() {
                merged.theme = old.theme.clone();
                merged.follow_system_theme = old.follow_system_theme;
            }
            if self.normal_color.is_some() {
                merged.comment.normal_color = old.comment.normal_color.clone();
            }
            if self.highlight_color_text.is_some() {
                merged.comment.highlight_color_text = old.comment.highlight_color_text.clone();
            }
            if self.comment_color.is_some() {
                merged.comment.color = old.comment.color.clone();
            }
            if self.highlight_comment_color.is_some() {
                merged.comment.highlight_color = old.comment.highlight_color.clone();
            }
            if self.comment_size_factor.is_some() {
                merged.comment.size_factor = old.comment.size_factor;
            }
            self.base = Some(merged);
        } else if base.is_some() {
            self.base = base;
        }
    }

    pub fn base(&self) -> Option<&ClassicUiState> {
        self.base.as_ref()
    }

    /// 有没有未应用的外观修改。
    pub fn is_dirty(&self) -> bool {
        // 颜色按十六进制值比（大小写不敏感）：主题里写 #164B73、控件回填 #164b73 是同一个颜色
        let differs = |draft: &Option<String>, base: Option<&String>| match (draft, base) {
            (None, _) => false,
            (Some(value), Some(base)) => !value.eq_ignore_ascii_case(base),
            (Some(_), None) => true,
        };
        let comment = self.base.as_ref().map(|base| &base.comment);
        let font_dirty = match (&self.font, &self.base) {
            (None, _) => false,
            (Some(font), Some(base)) => *font != base.font,
            (Some(_), None) => true,
        };
        font_dirty
            || match (&self.theme, &self.base) {
                (None, _) => false,
                (Some(theme), Some(base)) => *theme != base.theme || base.follow_system_theme,
                (Some(_), None) => true,
            }
            || differs(
                &self.normal_color,
                comment.and_then(|c| c.normal_color.as_ref()),
            )
            || differs(
                &self.highlight_color_text,
                comment.and_then(|c| c.highlight_color_text.as_ref()),
            )
            || differs(&self.comment_color, comment.and_then(|c| c.color.as_ref()))
            || differs(
                &self.highlight_comment_color,
                comment.and_then(|c| c.highlight_color.as_ref()),
            )
            || match (
                self.comment_size_factor,
                comment.and_then(|c| c.size_factor),
            ) {
                (None, _) => false,
                (Some(value), base) => Some(value) != base,
            }
    }

    /// 记一处字体修改；与现状相同就清掉（不再算脏）。
    pub fn set_font(&mut self, font: FontDesc) {
        self.font = match &self.base {
            Some(base) if font == base.font => None,
            _ => Some(font),
        };
    }

    /// 记一次主题选择；`None` 或选的就是当前主题都表示不动。
    pub fn set_theme(&mut self, name: Option<String>) {
        self.theme = match (name, &self.base) {
            (Some(name), Some(base)) if name == base.theme && !base.follow_system_theme => None,
            (name, _) => name,
        };
    }

    pub fn set_normal_color(&mut self, color: Option<String>) {
        self.normal_color = normalize(
            color,
            self.base
                .as_ref()
                .and_then(|b| b.comment.normal_color.clone()),
        );
    }

    pub fn set_highlight_color_text(&mut self, color: Option<String>) {
        self.highlight_color_text = normalize(
            color,
            self.base
                .as_ref()
                .and_then(|b| b.comment.highlight_color_text.clone()),
        );
    }

    pub fn normal_color(&self) -> Option<&String> {
        self.normal_color.as_ref()
    }

    pub fn highlight_color_text(&self) -> Option<&String> {
        self.highlight_color_text.as_ref()
    }

    pub fn set_comment_color(&mut self, color: Option<String>) {
        self.comment_color = normalize(
            color,
            self.base.as_ref().and_then(|b| b.comment.color.clone()),
        );
    }

    pub fn set_highlight_comment_color(&mut self, color: Option<String>) {
        self.highlight_comment_color = normalize(
            color,
            self.base
                .as_ref()
                .and_then(|b| b.comment.highlight_color.clone()),
        );
    }

    pub fn set_comment_size_factor(&mut self, factor: Option<u32>) {
        self.comment_size_factor = match (factor, &self.base) {
            (Some(value), Some(base)) if base.comment.size_factor == Some(value) => None,
            (value, _) => value,
        };
    }

    /// 界面回填用的生效值：草稿优先，其次现状，最后 classicui 缺省。
    pub fn font(&self) -> FontDesc {
        self.font
            .clone()
            .or_else(|| self.base.as_ref().map(|base| base.font.clone()))
            .unwrap_or_default()
    }

    pub fn comment_color(&self) -> Option<&String> {
        self.comment_color.as_ref()
    }

    pub fn highlight_comment_color(&self) -> Option<&String> {
        self.highlight_comment_color.as_ref()
    }

    pub fn comment_size_factor(&self) -> Option<u32> {
        self.comment_size_factor
    }

    pub fn theme(&self) -> Option<&String> {
        self.theme.as_ref()
    }

    /// 应用成功或撤销：清掉草稿，基准保留。
    pub fn clear(&mut self) {
        self.normal_color = None;
        self.highlight_color_text = None;
        self.font = None;
        self.theme = None;
        self.comment_color = None;
        self.highlight_comment_color = None;
        self.comment_size_factor = None;
    }

    /// 打包给保存管线。
    pub fn values(&self) -> AppearanceValues {
        AppearanceValues {
            normal_color: self.normal_color.clone(),
            highlight_color_text: self.highlight_color_text.clone(),
            font: self.font.clone(),
            theme: self.theme.clone(),
            comment_color: self.comment_color.clone(),
            highlight_comment_color: self.highlight_comment_color.clone(),
            comment_size_factor: self.comment_size_factor,
        }
    }
}

/// 与现状相同的取值清成 `None`（不再算脏）。
fn normalize(value: Option<String>, base: Option<String>) -> Option<String> {
    match (value, base) {
        (Some(value), Some(base)) if value.eq_ignore_ascii_case(&base) => None,
        (value, _) => value,
    }
}

#[cfg(test)]
mod tests;
