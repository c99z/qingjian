//! 外观页的静态预览：用 GTK 控件 + CSS 画一个示意候选面板（不接渲染器、不碰 Fcitx5）。
//! 只反映本地草稿：字体、释义颜色与字号比例、横竖排；面板的真实配色由主题决定，预览不模拟。

use adw::prelude::*;
use gtk::gdk;
use qingjian_platform::LayoutMode;

use crate::adapters::fcitx5::FontWeight;

mod model;
pub use model::PreviewModel;

const CANDIDATES: [&str; 5] = ["你好", "拟定", "泥泞", "尼姑", "你我"];
const ANNOTATIONS: [&str; 5] = ["hello", "to draft", "", "", ""];

pub struct Preview {
    root: gtk::Box,

    provider: gtk::CssProvider,

    candidates: gtk::Box,

    /// （行容器，候选标签，释义标签）；第一行是高亮行。
    rows: Vec<(gtk::Box, gtk::Label, gtk::Label)>,

    /// 横排时高亮候选的释义单独一行。
    horizontal_annotation: gtk::Label,
}

impl Preview {
    pub fn new() -> Self {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 6);
        root.add_css_class("qj-preview");
        root.set_margin_top(12);
        root.set_margin_bottom(12);
        root.set_margin_start(12);
        root.set_margin_end(12);

        let preedit = gtk::Label::new(Some("ni hao"));
        preedit.add_css_class("qj-preedit");
        preedit.set_xalign(0.0);
        root.append(&preedit);

        let candidates = gtk::Box::new(gtk::Orientation::Vertical, 2);
        let mut rows = Vec::new();
        for (index, (text, annotation)) in CANDIDATES.iter().zip(ANNOTATIONS).enumerate() {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            row.add_css_class("qj-candidate-row");
            if index == 0 {
                row.add_css_class("qj-highlight");
            }
            let candidate = gtk::Label::new(Some(&format!("{} {}", index + 1, text)));
            candidate.set_xalign(0.0);
            candidate.set_hexpand(true);
            row.append(&candidate);
            let annotation = gtk::Label::new(Some(annotation));
            annotation.add_css_class("qj-annotation");
            row.append(&annotation);
            candidates.append(&row);
            rows.push((row, candidate, annotation));
        }
        root.append(&candidates);

        let horizontal_annotation = gtk::Label::new(Some(ANNOTATIONS[0]));
        horizontal_annotation.add_css_class("qj-annotation");
        horizontal_annotation.add_css_class("qj-horizontal-annotation");
        horizontal_annotation.set_xalign(0.0);
        root.append(&horizontal_annotation);

        let provider = gtk::CssProvider::new();
        if let Some(display) = gdk::Display::default() {
            gtk::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
        Self {
            root,
            provider,
            candidates,
            rows,
            horizontal_annotation,
        }
    }

    pub fn widget(&self) -> &gtk::Box {
        &self.root
    }

    pub fn update(&self, model: &PreviewModel) {
        let vertical = model.layout == LayoutMode::Vertical;
        self.candidates.set_orientation(if vertical {
            gtk::Orientation::Vertical
        } else {
            gtk::Orientation::Horizontal
        });
        for (_, _, annotation) in &self.rows {
            annotation.set_visible(vertical && !annotation.text().is_empty());
        }
        self.horizontal_annotation.set_visible(!vertical);

        let comment = model.comment_color.as_deref().unwrap_or("#6b7280");
        let highlight = model
            .highlight_comment_color
            .as_deref()
            .unwrap_or("#dbeafe");
        let annotation_size = (model.font.size * model.comment_factor / 100).max(6);
        let weight = match model.font.weight {
            FontWeight::Regular => "normal",
            FontWeight::Bold => "bold",
        };
        let css = format!(
            ".qj-preview {{ background-color: #fdfdfd; border: 1px solid #d6d6d6; border-radius: 10px; }}
             .qj-preview label {{ font-family: {}; font-size: {}pt; font-weight: {}; color: {normal}; }}
             .qj-preview .qj-preedit {{ color: #777777; }}
             .qj-candidate-row {{ border-radius: 6px; padding: 2px 8px; }}
             .qj-highlight {{ background-color: #3584e4; }}
             .qj-preview .qj-highlight label {{ color: {text_highlight}; }}
             .qj-preview .qj-annotation {{ font-size: {annotation_size}pt; color: {comment}; }}
             .qj-preview .qj-highlight .qj-annotation, .qj-preview .qj-horizontal-annotation {{ color: {highlight}; }}",
            css_family(&model.font.family),
            model.font.size,
            weight,
            normal = model.normal_color,
            text_highlight = model.highlight_color_text,
        );
        self.provider.load_from_string(&css);
    }
}

/// CSS 字体名单：名字加引号，引号与反斜杠转义。
fn css_family(family: &str) -> String {
    format!("\"{}\"", family.replace('\\', "\\\\").replace('"', "\\\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn css_family_is_quoted_and_escaped() {
        assert_eq!(css_family("Noto Sans CJK SC"), "\"Noto Sans CJK SC\"");
        assert_eq!(css_family("we\"ird"), "\"we\\\"ird\"");
    }
}
