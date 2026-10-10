//! 通用页：每页候选数、输入方案、拼音显示位置、英文候选、中文优先、Shift 字母行为。
//! 全部落在 `[general]` 分节；选项列表直接由共享配置枚举生成。

use adw::prelude::*;
use qingjian_platform::{Config, PreeditMode, Scheme, ShiftLetter};

use super::{
    PageContext, SettingsPage, SyncGuard, clear_row_error, combo_row, mark_row_error, switch_row,
};

pub struct GeneralPage {
    page: adw::PreferencesPage,

    syncing: SyncGuard,

    page_size: adw::SpinRow,

    scheme: adw::ComboRow,

    preedit: adw::ComboRow,

    english_candidates: adw::SwitchRow,

    chinese_first: adw::SwitchRow,

    shift_letter: adw::ComboRow,

    /// 字段路径 → 行，校验失败时标注用。
    fields: Vec<(&'static str, adw::PreferencesRow)>,
}

impl GeneralPage {
    pub fn new(ctx: &PageContext) -> Self {
        let page = adw::PreferencesPage::new();
        let syncing = SyncGuard::default();

        let composing = adw::PreferencesGroup::new();
        composing.set_title("输入");

        let page_size = adw::SpinRow::with_range(1.0, 9.0, 1.0);
        page_size.set_title("每页候选数");
        page_size.set_subtitle("数字键只有 1–9");
        {
            let ctx = ctx.clone();
            let syncing = syncing.clone();
            page_size.connect_notify_local(Some("value"), move |row, _| {
                if !syncing.get() {
                    ctx.set("general", "page_size", row.value() as i64);
                }
            });
        }
        composing.add(&page_size);

        let scheme_labels: Vec<&str> = Scheme::ALL.iter().map(|s| s.label()).collect();
        let ctx_scheme = ctx.clone();
        let scheme = combo_row(
            "输入方案",
            &scheme_labels,
            move |index| {
                ctx_scheme.set("general", "scheme", Scheme::ALL[index as usize].key());
            },
            &syncing,
        );
        scheme.set_subtitle("双拼与注音下表达式模式没有入口；微软、搜狗方案的 ; 是 ing 的韵母键");
        composing.add(&scheme);

        let preedit_labels: Vec<&str> = PreeditMode::ALL.iter().map(|m| m.label()).collect();
        let ctx_preedit = ctx.clone();
        let preedit = combo_row(
            "拼音显示位置",
            &preedit_labels,
            move |index| {
                ctx_preedit.set("general", "preedit", PreeditMode::ALL[index as usize].key());
            },
            &syncing,
        );
        preedit.set_subtitle("组句中的拼音显示在行内、候选窗口还是两处都显示");
        composing.add(&preedit);
        page.add(&composing);

        let candidates = adw::PreferencesGroup::new();
        candidates.set_title("候选");

        let ctx_english = ctx.clone();
        let english_candidates = switch_row(
            "英文候选",
            "青简英文模式下给英文补全与拼错纠正；Fcitx5 的键盘模式直接输入字母",
            move |on| ctx_english.set("general", "english_candidates", on),
            &syncing,
        );
        candidates.add(&english_candidates);

        let ctx_chinese = ctx.clone();
        let chinese_first = switch_row(
            "中文优先",
            "中英混输时中文候选总排在英文词前面；缺省关：拼音不像话的输入（hello）英文词排第一",
            move |on| ctx_chinese.set("general", "chinese_first", on),
            &syncing,
        );
        candidates.add(&chinese_first);

        let shift_labels: Vec<&str> = ShiftLetter::ALL.iter().map(|s| s.label()).collect();
        let ctx_shift = ctx.clone();
        let shift_letter = combo_row(
            "Shift 字母行为",
            &shift_labels,
            move |index| {
                ctx_shift.set(
                    "general",
                    "shift_letter",
                    ShiftLetter::ALL[index as usize].key(),
                );
            },
            &syncing,
        );
        shift_letter.set_subtitle(
            "中文模式下按住 Shift 敲的字母：交给应用，还是收进组句（Cpan 与 cpan 一样出「C盘」）",
        );
        candidates.add(&shift_letter);
        page.add(&candidates);

        let fields = vec![
            ("general.page_size", page_size.clone().upcast()),
            ("general.scheme", scheme.clone().upcast()),
            ("general.preedit", preedit.clone().upcast()),
            (
                "general.english_candidates",
                english_candidates.clone().upcast(),
            ),
            ("general.chinese_first", chinese_first.clone().upcast()),
            ("general.shift_letter", shift_letter.clone().upcast()),
        ];
        Self {
            page,
            syncing,
            page_size,
            scheme,
            preedit,
            english_candidates,
            chinese_first,
            shift_letter,
            fields,
        }
    }
}

impl SettingsPage for GeneralPage {
    fn title(&self) -> &'static str {
        "通用"
    }

    fn widget(&self) -> &adw::PreferencesPage {
        &self.page
    }

    fn refresh(&self, config: &Config) {
        self.syncing.set(true);
        let general = &config.general;
        self.page_size.set_value(general.page_size() as f64);
        let scheme = general.scheme();
        self.scheme.set_selected(
            Scheme::ALL
                .iter()
                .position(|s| *s == scheme)
                .unwrap_or_default() as u32,
        );
        self.preedit.set_selected(
            PreeditMode::ALL
                .iter()
                .position(|m| *m == general.preedit)
                .unwrap_or_default() as u32,
        );
        self.english_candidates
            .set_active(general.english_candidates);
        self.chinese_first.set_active(general.chinese_first);
        self.shift_letter.set_selected(
            ShiftLetter::ALL
                .iter()
                .position(|s| *s == general.shift_letter)
                .unwrap_or_default() as u32,
        );
        self.syncing.set(false);
    }

    fn clear_errors(&self) {
        for (_, row) in &self.fields {
            clear_row_error(row);
        }
    }

    fn mark_error(&self, field: &str, message: &str) -> bool {
        let Some((_, row)) = self.fields.iter().find(|(path, _)| *path == field) else {
            return false;
        };
        mark_row_error(row, message);
        true
    }
}
