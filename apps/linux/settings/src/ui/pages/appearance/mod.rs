//! 「候选与外观」页：横竖排（青简 `general.layout`），以及 Fcitx5 经典面板的主题、字体、
//! 释义样式（颜色与字号比例）、静态预览与实际输入测试框。
//!
//! 青简侧字段走共享草稿管线；Fcitx5 侧字段进 [`AppearanceDraft`]（不进 `config.toml`），
//! 应用时由适配层写 classicui 并读回核对。Fcitx5 没跑或经典面板被禁用时，Fcitx5 侧控制项禁用，
//! 青简侧照常可改。

mod preview;
mod probe_outcome;
use probe_outcome::ProbeOutcome;

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use gtk::{gdk, gio, glib};
use qingjian_platform::{Config, LayoutMode};

use crate::adapters::fcitx5::{
    ClassicUiPanel, Fcitx5Client, Fcitx5Error, FontDesc, FontWeight, ThemeOption, color,
    theme_options,
};
use crate::state::AppearanceDraft;

use super::{PageContext, SettingsPage, SyncGuard, clear_row_error, combo_row, mark_row_error};
use preview::{Preview, PreviewModel};

/// 释义字号比例下拉的选项（`None` = 跟随主题）。
const FACTOR_OPTIONS: [(&str, Option<u32>); 6] = [
    ("跟随主题", None),
    ("与中文同字号（100%）", Some(100)),
    ("90%", Some(90)),
    ("80%", Some(80)),
    ("75%（Fcitx5 默认）", Some(75)),
    ("60%", Some(60)),
];

/// 释义颜色展开时的起始色（当前主题没写这两个字段时的示意缺省）。
const FALLBACK_COMMENT_COLOR: &str = "#666666";
const FALLBACK_HIGHLIGHT_COMMENT_COLOR: &str = "#ffffff";

pub struct AppearancePage {
    page: adw::PreferencesPage,

    ctx: PageContext,

    syncing: SyncGuard,

    layout: adw::ComboRow,

    fcitx5_status: adw::ActionRow,

    panel_status: adw::ActionRow,

    theme: adw::ComboRow,

    theme_options: Rc<RefCell<Vec<ThemeOption>>>,

    font_family: adw::EntryRow,

    font_size: adw::SpinRow,

    font_weight: adw::ComboRow,

    factor: adw::ComboRow,

    colors: adw::ExpanderRow,

    normal_color: gtk::ColorDialogButton,

    highlight_color_text: gtk::ColorDialogButton,

    comment_color: gtk::ColorDialogButton,

    highlight_comment_color: gtk::ColorDialogButton,

    preview: Preview,

    /// 自持弱引用：探测回调与「保存后重新探测」用，不留 Rc 环。
    weak: RefCell<std::rc::Weak<Self>>,

    probe_generation: std::cell::Cell<u64>,
}

impl AppearancePage {
    pub fn new(ctx: &PageContext) -> Rc<Self> {
        let page = adw::PreferencesPage::new();
        let syncing = SyncGuard::default();

        let arrangement = adw::PreferencesGroup::new();
        arrangement.set_title("排列");
        let layout_labels: Vec<&str> = LayoutMode::ALL.iter().map(|m| m.label()).collect();
        let ctx_layout = ctx.clone();
        let layout = combo_row(
            "候选排列",
            &layout_labels,
            move |index| ctx_layout.set("general", "layout", LayoutMode::ALL[index as usize].key()),
            &syncing,
        );
        layout.set_subtitle("竖排一行一个候选；横排中文在上、释义在下。应用后生效");
        arrangement.add(&layout);
        page.add(&arrangement);

        let classic = adw::PreferencesGroup::new();
        classic.set_title("Fcitx5 经典面板");
        classic.set_description(Some(
            "以下设置作用于 Fcitx5 经典面板，使用同一面板的其他输入法也会受影响；\
             点击「应用」后生效",
        ));
        let refresh = gtk::Button::from_icon_name("view-refresh-symbolic");
        refresh.set_tooltip_text(Some("重新检测 Fcitx5"));
        refresh.set_valign(gtk::Align::Center);
        refresh.add_css_class("flat");
        classic.set_header_suffix(Some(&refresh));

        let fcitx5_status = adw::ActionRow::new();
        fcitx5_status.set_title("Fcitx5");
        fcitx5_status.set_subtitle("检测中…");
        classic.add(&fcitx5_status);
        let panel_status = adw::ActionRow::new();
        panel_status.set_title("候选面板");
        classic.add(&panel_status);

        let theme_options = Rc::new(RefCell::new(vec![ThemeOption {
            label: "保持当前主题".to_owned(),
            name: None,
        }]));
        let theme = adw::ComboRow::new();
        theme.set_title("主题");
        theme.set_subtitle("默认保持当前主题；选择预设后固定深浅色");
        classic.add(&theme);

        let font_family = adw::EntryRow::new();
        font_family.set_title("候选字体（字族名）");
        classic.add(&font_family);
        let font_size = adw::SpinRow::with_range(6.0, 32.0, 1.0);
        font_size.set_title("字号");
        classic.add(&font_size);
        let weight_labels: Vec<&str> = FontWeight::ALL.iter().map(|w| w.label()).collect();
        let font_weight = combo_row("字重", &weight_labels, |_| {}, &syncing);
        classic.add(&font_weight);
        page.add(&classic);

        let comment = adw::PreferencesGroup::new();
        comment.set_title("释义样式");
        comment.set_description(Some(
            "释义的颜色与字号比例写在主题里：定制会在应用时把所选主题复制成青简管理的「青简定制」主题再改，\
             原主题不动。横排的中文与释义共用候选字号和颜色；独立释义样式用于竖排",
        ));
        let factor_labels: Vec<&str> = FACTOR_OPTIONS.iter().map(|(label, _)| *label).collect();
        let factor = combo_row("释义字号比例", &factor_labels, |_| {}, &syncing);
        factor.set_subtitle("释义相对候选的字号；选「与中文同字号」则释义与中文同字号、同字重");
        comment.add(&factor);

        let colors = adw::ExpanderRow::new();
        colors.set_title("自定义中文与释义颜色");
        colors.set_show_enable_switch(true);
        colors.set_subtitle("缺省跟随主题色；展开后应用时会生成定制主题");
        let normal_color = color_button(&colors, "普通候选中文颜色");
        let highlight_color_text = color_button(&colors, "高亮候选中文颜色");
        let normal_row = adw::ActionRow::new();
        normal_row.set_title("普通候选的释义颜色");
        let comment_color = gtk::ColorDialogButton::new(Some(gtk::ColorDialog::new()));
        comment_color.set_valign(gtk::Align::Center);
        normal_row.add_suffix(&comment_color);
        normal_row.set_activatable_widget(Some(&comment_color));
        colors.add_row(&normal_row);
        let highlight_row = adw::ActionRow::new();
        highlight_row.set_title("高亮候选的释义颜色");
        let highlight_comment_color = gtk::ColorDialogButton::new(Some(gtk::ColorDialog::new()));
        highlight_comment_color.set_valign(gtk::Align::Center);
        highlight_row.add_suffix(&highlight_comment_color);
        highlight_row.set_activatable_widget(Some(&highlight_comment_color));
        colors.add_row(&highlight_row);
        comment.add(&colors);
        page.add(&comment);

        let preview_group = adw::PreferencesGroup::new();
        preview_group.set_title("预览");
        preview_group.set_description(Some("示意效果，只反映本地草稿；真实配色由所选主题决定"));
        let preview = Preview::new();
        preview_group.add(preview.widget());
        page.add(&preview_group);

        let test_group = adw::PreferencesGroup::new();
        test_group.set_title("实际输入测试");
        test_group.set_description(Some(
            "在这个输入框里打字会弹出真实候选面板；应用外观后在这里验证字体、颜色与排布",
        ));
        let test_entry = gtk::Entry::new();
        test_entry.set_placeholder_text(Some("输入拼音试试…"));
        test_group.add(&test_entry);
        page.add(&test_group);

        let this = Rc::new(Self {
            page,
            ctx: ctx.clone(),
            syncing,
            layout,
            fcitx5_status,
            panel_status,
            theme,
            theme_options,
            font_family,
            font_size,
            font_weight,
            factor,
            colors,
            normal_color,
            highlight_color_text,
            comment_color,
            highlight_comment_color,
            preview,
            weak: RefCell::new(std::rc::Weak::new()),
            probe_generation: std::cell::Cell::new(0),
        });
        *this.weak.borrow_mut() = Rc::downgrade(&this);
        this.connect_controls();
        // 进页面与点刷新都重新探测
        {
            let weak = Rc::downgrade(&this);
            this.page.connect_map(move |_| {
                if let Some(this) = weak.upgrade() {
                    this.probe();
                }
            });
        }
        {
            let weak = Rc::downgrade(&this);
            refresh.connect_clicked(move |_| {
                if let Some(this) = weak.upgrade() {
                    this.probe();
                }
            });
        }
        this.probe();
        this
    }

    /// 记一处 Fcitx5 侧草稿修改并刷新预览与底栏。
    fn edit(&self, change: impl FnOnce(&mut AppearanceDraft)) {
        change(self.ctx.store.borrow_mut().appearance_mut());
        (self.ctx.notify)();
        self.refresh_preview();
    }
}

impl SettingsPage for AppearancePage {
    fn title(&self) -> &'static str {
        "候选与外观"
    }

    fn widget(&self) -> &adw::PreferencesPage {
        &self.page
    }

    fn refresh(&self, config: &Config) {
        self.syncing.set(true);
        self.layout.set_selected(
            LayoutMode::ALL
                .iter()
                .position(|m| *m == config.general.layout)
                .unwrap_or_default() as u32,
        );
        self.syncing.set(false);
        self.refresh_appearance();
    }

    fn clear_errors(&self) {
        clear_row_error(&self.layout);
    }

    fn mark_error(&self, field: &str, message: &str) -> bool {
        if field != "general.layout" {
            return false;
        }
        mark_row_error(&self.layout, message);
        true
    }

    fn has_invalid_input(&self) -> bool {
        self.font_family.text().trim().is_empty() || self.font_family.text().contains(['\n', '\r'])
    }

    /// 外观写进 Fcitx5 后重新探测：基准更新为刚写入的实际值，控件显示与面板一致。
    fn on_saved(&self) {
        self.probe();
    }
}

/// 控件信号与探测；拆出来免得 `new` 过长。
impl AppearancePage {
    fn connect_controls(self: &Rc<Self>) {
        // 信号回调全部走弱引用：页面与窗口同寿命，但不在控件树里留 Rc 环
        // 主题下拉：模型随探测结果重建，选中项映射到主题名（None = 保持当前主题）
        {
            let weak = Rc::downgrade(self);
            self.theme.connect_selected_notify(move |row| {
                let Some(this) = weak.upgrade() else { return };
                if this.syncing.get() {
                    return;
                }
                let name = this
                    .theme_options
                    .borrow()
                    .get(row.selected() as usize)
                    .and_then(|option| option.name.clone());
                this.edit(|draft| draft.set_theme(name));
            });
        }
        // 字体三件套：任一改动都按控件当前值合成 FontDesc 进草稿
        {
            let weak = Rc::downgrade(self);
            self.font_family.connect_changed(move |_| {
                let Some(this) = weak.upgrade() else { return };
                if !this.syncing.get() {
                    this.font_edited();
                }
            });
        }
        {
            let weak = Rc::downgrade(self);
            self.font_size
                .connect_notify_local(Some("value"), move |_, _| {
                    let Some(this) = weak.upgrade() else { return };
                    if !this.syncing.get() {
                        this.font_edited();
                    }
                });
        }
        {
            let weak = Rc::downgrade(self);
            self.font_weight.connect_selected_notify(move |_| {
                let Some(this) = weak.upgrade() else { return };
                if !this.syncing.get() {
                    this.font_edited();
                }
            });
        }
        {
            let weak = Rc::downgrade(self);
            self.factor.connect_selected_notify(move |row| {
                let Some(this) = weak.upgrade() else { return };
                if this.syncing.get() {
                    return;
                }
                let (_, value) = FACTOR_OPTIONS[row.selected() as usize];
                this.edit(|draft| draft.set_comment_size_factor(value));
            });
        }
        // 颜色展开 = 启用定制：把两个颜色草稿设成当前生效值；收起 = 回到跟随主题
        {
            let weak = Rc::downgrade(self);
            self.colors.connect_enable_expansion_notify(move |row| {
                let Some(this) = weak.upgrade() else { return };
                if this.syncing.get() {
                    return;
                }
                if row.enables_expansion() {
                    let (normal, highlight) = this.effective_colors();
                    let text_normal = rgba_hex(&this.normal_color.rgba());
                    let text_highlight = rgba_hex(&this.highlight_color_text.rgba());
                    this.edit(|draft| {
                        draft.set_normal_color(Some(text_normal));
                        draft.set_highlight_color_text(Some(text_highlight));
                        draft.set_comment_color(Some(normal));
                        draft.set_highlight_comment_color(Some(highlight));
                    });
                } else {
                    this.edit(|draft| {
                        draft.set_normal_color(None);
                        draft.set_highlight_color_text(None);
                        draft.set_comment_color(None);
                        draft.set_highlight_comment_color(None);
                    });
                }
            });
        }
        {
            let weak = Rc::downgrade(self);
            self.comment_color.connect_rgba_notify(move |button| {
                let Some(this) = weak.upgrade() else { return };
                if this.syncing.get() {
                    return;
                }
                let hex = rgba_hex(&button.rgba());
                this.edit(|draft| draft.set_comment_color(Some(hex)));
            });
        }
        {
            let weak = Rc::downgrade(self);
            self.highlight_comment_color
                .connect_rgba_notify(move |button| {
                    let Some(this) = weak.upgrade() else { return };
                    if this.syncing.get() {
                        return;
                    }
                    let hex = rgba_hex(&button.rgba());
                    this.edit(|draft| draft.set_highlight_comment_color(Some(hex)));
                });
        }
        for (button, highlighted) in [
            (&self.normal_color, false),
            (&self.highlight_color_text, true),
        ] {
            let weak = Rc::downgrade(self);
            button.connect_rgba_notify(move |button| {
                let Some(this) = weak.upgrade() else { return };
                if this.syncing.get() {
                    return;
                }
                let hex = rgba_hex(&button.rgba());
                this.edit(|draft| {
                    if highlighted {
                        draft.set_highlight_color_text(Some(hex));
                    } else {
                        draft.set_normal_color(Some(hex));
                    }
                });
            });
        }
        // 横竖排也影响预览
        {
            let weak = Rc::downgrade(self);
            self.layout.connect_selected_notify(move |_| {
                let Some(this) = weak.upgrade() else { return };
                if !this.syncing.get() {
                    this.refresh_preview();
                }
            });
        }
    }

    fn font_edited(&self) {
        let font = self.font_from_controls();
        self.edit(|draft| draft.set_font(font));
    }

    fn font_from_controls(&self) -> FontDesc {
        let family = self.font_family.text().trim().to_owned();
        FontDesc {
            family: if family.is_empty() {
                "Sans".to_owned()
            } else {
                family
            },
            weight: FontWeight::ALL[self.font_weight.selected() as usize],
            size: self.font_size.value() as u32,
        }
    }

    /// 释义颜色的当前生效值（草稿 → 主题现状 → 示意缺省）。
    fn effective_colors(&self) -> (String, String) {
        let store = self.ctx.store.borrow();
        let appearance = store.appearance();
        let normal = appearance
            .comment_color()
            .cloned()
            .or_else(|| appearance.base().and_then(|b| b.comment.color.clone()))
            .unwrap_or_else(|| FALLBACK_COMMENT_COLOR.to_owned());
        let highlight = appearance
            .highlight_comment_color()
            .cloned()
            .or_else(|| {
                appearance
                    .base()
                    .and_then(|b| b.comment.highlight_color.clone())
            })
            .unwrap_or_else(|| FALLBACK_HIGHLIGHT_COMMENT_COLOR.to_owned());
        (normal, highlight)
    }

    /// 从草稿与基准回填 Fcitx5 侧控件并刷新预览。
    fn refresh_appearance(&self) {
        let store = self.ctx.store.borrow();
        let appearance = store.appearance();
        self.syncing.set(true);

        let options = match appearance.base() {
            Some(base) => theme_options(&base.themes, &base.theme),
            None => vec![ThemeOption {
                label: "保持当前主题".to_owned(),
                name: None,
            }],
        };
        let labels: Vec<&str> = options.iter().map(|o| o.label.as_str()).collect();
        self.theme.set_model(Some(&gtk::StringList::new(&labels)));
        let selected = appearance
            .theme()
            .and_then(|name| options.iter().position(|o| o.name.as_deref() == Some(name)))
            .unwrap_or(0);
        self.theme.set_selected(selected as u32);
        *self.theme_options.borrow_mut() = options;

        let font = appearance.font();
        self.font_family.set_text(&font.family);
        self.font_size.set_value(font.size as f64);
        self.font_weight.set_selected(
            FontWeight::ALL
                .iter()
                .position(|w| *w == font.weight)
                .unwrap_or_default() as u32,
        );

        let factor = appearance.comment_size_factor();
        self.factor.set_selected(
            FACTOR_OPTIONS
                .iter()
                .position(|(_, value)| *value == factor)
                .unwrap_or_default() as u32,
        );

        let custom_colors = appearance.normal_color().is_some()
            || appearance.highlight_color_text().is_some()
            || appearance.comment_color().is_some()
            || appearance.highlight_comment_color().is_some();
        let (text, highlighted) = self.effective_text_colors(appearance);
        if let Some(rgba) = hex_rgba(&text) {
            self.normal_color.set_rgba(&rgba);
        }
        if let Some(rgba) = hex_rgba(&highlighted) {
            self.highlight_color_text.set_rgba(&rgba);
        }
        self.colors.set_enable_expansion(custom_colors);
        let (normal, highlight) = self.effective_colors_of(appearance);
        if let Some(rgba) = hex_rgba(&normal) {
            self.comment_color.set_rgba(&rgba);
        }
        if let Some(rgba) = hex_rgba(&highlight) {
            self.highlight_comment_color.set_rgba(&rgba);
        }

        self.syncing.set(false);
        drop(store);
        self.refresh_preview();
    }

    fn effective_colors_of(&self, appearance: &AppearanceDraft) -> (String, String) {
        let normal = appearance
            .comment_color()
            .cloned()
            .or_else(|| appearance.base().and_then(|b| b.comment.color.clone()))
            .unwrap_or_else(|| FALLBACK_COMMENT_COLOR.to_owned());
        let highlight = appearance
            .highlight_comment_color()
            .cloned()
            .or_else(|| {
                appearance
                    .base()
                    .and_then(|b| b.comment.highlight_color.clone())
            })
            .unwrap_or_else(|| FALLBACK_HIGHLIGHT_COMMENT_COLOR.to_owned());
        (normal, highlight)
    }

    fn effective_text_colors(&self, appearance: &AppearanceDraft) -> (String, String) {
        let normal = appearance
            .normal_color()
            .cloned()
            .or_else(|| {
                appearance
                    .base()
                    .and_then(|b| b.comment.normal_color.clone())
            })
            .unwrap_or_else(|| "#000000".to_owned());
        let highlight = appearance
            .highlight_color_text()
            .cloned()
            .or_else(|| {
                appearance
                    .base()
                    .and_then(|b| b.comment.highlight_color_text.clone())
            })
            .unwrap_or_else(|| "#ffffff".to_owned());
        (normal, highlight)
    }

    fn refresh_preview(&self) {
        let store = self.ctx.store.borrow();
        let appearance = store.appearance();
        let (comment_color, highlight_color) = self.effective_colors_of(appearance);
        let factor = appearance
            .comment_size_factor()
            .or_else(|| appearance.base().and_then(|b| b.comment.size_factor))
            .unwrap_or(75);
        let (normal_color, highlight_color_text) = self.effective_text_colors(appearance);
        let model = PreviewModel {
            normal_color,
            highlight_color_text,
            layout: LayoutMode::ALL[self.layout.selected() as usize],
            font: appearance.font(),
            comment_color: appearance.comment_color().cloned().or(Some(comment_color)),
            highlight_comment_color: appearance
                .highlight_comment_color()
                .cloned()
                .or(Some(highlight_color)),
            comment_factor: factor,
        };
        drop(store);
        self.preview.update(&model);
    }

    /// 后台探测 Fcitx5：面板状态 + 经典面板当前配置，回主线程更新基准与状态行。
    fn probe(&self) {
        self.fcitx5_status.set_subtitle("检测中…");
        let generation = self.probe_generation.get() + 1;
        self.probe_generation.set(generation);
        let weak = self.weak.borrow().clone();
        glib::MainContext::default().spawn_local(async move {
            let outcome = gio::spawn_blocking(probe_fcitx5).await;
            if let Some(this) = weak.upgrade()
                && this.probe_generation.get() == generation
            {
                this.apply_probe(
                    outcome.unwrap_or(ProbeOutcome::Failed("后台任务中断".to_owned())),
                );
            }
        });
    }

    fn apply_probe(&self, outcome: ProbeOutcome) {
        let sensitive;
        match outcome {
            ProbeOutcome::NotRunning => {
                self.ctx.store.borrow_mut().appearance_mut().set_base(None);
                self.fcitx5_status.set_subtitle("未运行");
                self.panel_status.set_subtitle(
                    "Fcitx5 未运行时无法读取与应用外观；已有的外观修改会保留在草稿中",
                );
                sensitive = false;
            }
            ProbeOutcome::Failed(message) => {
                self.ctx.store.borrow_mut().appearance_mut().set_base(None);
                self.fcitx5_status
                    .set_subtitle(&format!("检测失败：{message}"));
                self.panel_status.set_subtitle("—");
                sensitive = false;
            }
            ProbeOutcome::Running { panels, state } => {
                self.fcitx5_status.set_subtitle("运行中");
                let mut detail = match panels.classic_ui {
                    ClassicUiPanel::Active => "经典面板已启用".to_owned(),
                    ClassicUiPanel::Disabled => {
                        "经典面板（classicui）被禁用：本页的外观设置不会生效".to_owned()
                    }
                    ClassicUiPanel::Missing => {
                        "未找到经典面板（classicui）：本页的外观设置不会生效".to_owned()
                    }
                };
                if state.is_none() && panels.classic_ui == ClassicUiPanel::Active {
                    detail.push_str("；经典面板配置读取失败");
                }
                if !panels.other_panels.is_empty() {
                    detail.push_str(&format!(
                        "；同时检测到其他界面插件（{}），若桌面由它显示候选，经典面板设置不生效",
                        panels.other_panels.join("、")
                    ));
                }
                self.panel_status.set_subtitle(&detail);
                sensitive = panels.classic_ui == ClassicUiPanel::Active && state.is_some();
                self.ctx.store.borrow_mut().appearance_mut().set_base(state);
            }
        }
        for row in self.fcitx5_rows() {
            row.set_sensitive(sensitive);
        }
        self.refresh_appearance();
        (self.ctx.notify)();
    }

    /// Fcitx5 侧的控制项（主题、字体、释义样式）。
    fn fcitx5_rows(&self) -> Vec<gtk::Widget> {
        vec![
            self.theme.clone().upcast(),
            self.font_family.clone().upcast(),
            self.font_size.clone().upcast(),
            self.font_weight.clone().upcast(),
            self.factor.clone().upcast(),
            self.colors.clone().upcast(),
        ]
    }
}

/// 后台线程里的探测：连不上总线分「未运行」与「其它失败」。
fn probe_fcitx5() -> ProbeOutcome {
    let client = match Fcitx5Client::connect() {
        Ok(client) => client,
        Err(Fcitx5Error::NotRunning) => return ProbeOutcome::NotRunning,
        Err(error) => return ProbeOutcome::Failed(error.user_message()),
    };
    match client.probe_panels() {
        Ok(panels) => ProbeOutcome::Running {
            panels,
            state: client.classic_ui().ok(),
        },
        Err(error) => ProbeOutcome::Failed(error.user_message()),
    }
}

/// 草稿里的 `#rrggbb` → 控件色。
fn hex_rgba(hex: &str) -> Option<gdk::RGBA> {
    let (r, g, b) = color::parse_hex(hex)?;
    Some(gdk::RGBA::new(
        r as f32 / 255.0,
        g as f32 / 255.0,
        b as f32 / 255.0,
        1.0,
    ))
}

/// 控件色 → `#rrggbb`（gdk 的 `to_string` 出来的是 `rgb(...)` 函数式写法，classicui 不认）。
fn rgba_hex(rgba: &gdk::RGBA) -> String {
    let channel = |value: f32| (value.clamp(0.0, 1.0) * 255.0).round() as u8;
    color::to_hex((
        channel(rgba.red()),
        channel(rgba.green()),
        channel(rgba.blue()),
    ))
}

fn color_button(parent: &adw::ExpanderRow, title: &str) -> gtk::ColorDialogButton {
    let row = adw::ActionRow::new();
    row.set_title(title);
    let button = gtk::ColorDialogButton::new(Some(gtk::ColorDialog::new()));
    button.set_valign(gtk::Align::Center);
    row.add_suffix(&button);
    row.set_activatable_widget(Some(&button));
    parent.add_row(&row);
    button
}
