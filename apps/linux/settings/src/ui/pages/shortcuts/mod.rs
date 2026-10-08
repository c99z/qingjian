//! 快捷键页：中英切换键、翻页键、第一 / 第二译词键、Shift+空格译词、删除候选、模式前缀，
//! 以及「恢复本页默认」（只改草稿，点应用后才保存）。字段冲突由 `settings::validate` 兜底。

mod modifier_row;

use std::rc::Rc;

use adw::prelude::*;
use qingjian_platform::{Config, PAGE_KEY_OPTIONS, ShortcutConfig, SwitchKey, SwitchKeys};

use modifier_row::ModifierRow;

use super::{
    PageContext, SettingsPage, SyncGuard, clear_row_error, combo_row, mark_row_error, switch_row,
};

/// 模式前缀键的可选字母，与 `qingjian_core::ModeKeys` 的候选一致（这三个字母不是拼音音节开头）。
const MODE_KEY_CANDIDATES: [char; 3] = ['v', 'u', 'i'];

pub struct ShortcutsPage {
    page: adw::PreferencesPage,

    ctx: PageContext,

    syncing: SyncGuard,

    /// 中英切换键勾选，顺序同 [`SwitchKey::ALL`]。
    switch_checks: Vec<gtk::CheckButton>,

    page_keys: adw::ComboRow,

    translation: ModifierRow,

    translation_second: ModifierRow,

    translation_space: ModifierRow,

    delete_candidate: ModifierRow,

    expression: adw::ComboRow,

    question: adw::ComboRow,

    question_mark: adw::SwitchRow,

    /// 字段路径 → 行（修饰键输入行走自己的本地校验，不在此列）。
    fields: Vec<(&'static str, adw::PreferencesRow)>,
}

impl ShortcutsPage {
    /// 「恢复本页默认」要回填全部控件，需要页面句柄，所以返回 `Rc`。
    pub fn new(ctx: &PageContext) -> Rc<Self> {
        let page = adw::PreferencesPage::new();
        let syncing = SyncGuard::default();

        let switching = adw::PreferencesGroup::new();
        switching.set_title("中英切换");
        switching.set_description(Some(
            "Linux 的中英切换由 Fcitx5 负责；这里的切换键只在 Windows 上生效",
        ));
        let mut switch_checks: Vec<gtk::CheckButton> = Vec::new();
        for key in SwitchKey::ALL {
            let row = adw::ActionRow::new();
            row.set_title(key.label());
            let check = gtk::CheckButton::new();
            check.set_valign(gtk::Align::Center);
            row.add_suffix(&check);
            row.set_activatable_widget(Some(&check));
            let checks_signal = switch_checks.clone();
            let ctx = ctx.clone();
            let syncing = syncing.clone();
            check.connect_toggled(move |_| {
                if syncing.get() {
                    return;
                }
                let mut keys = SwitchKeys::NONE;
                for (key, check) in SwitchKey::ALL.iter().zip(&checks_signal) {
                    keys = keys.with(*key, check.is_active());
                }
                ctx.set_array("shortcut", "switch_mode", &keys.config_values());
            });
            switch_checks.push(check);
            switching.add(&row);
        }
        page.add(&switching);

        let candidates = adw::PreferencesGroup::new();
        candidates.set_title("候选操作");
        candidates.set_description(Some(
            "译词与删候选都是「数字键 / 空格 + 修饰键」；修饰键写法如 shift+option（option 即 Alt）",
        ));

        let page_key_labels: Vec<String> = PAGE_KEY_OPTIONS
            .iter()
            .map(|pair| {
                let mut chars = pair.chars();
                format!(
                    "「{}」上页、「{}」下页",
                    chars.next().unwrap_or_default(),
                    chars.next().unwrap_or_default()
                )
            })
            .collect();
        let page_key_labels: Vec<&str> = page_key_labels.iter().map(String::as_str).collect();
        let ctx_keys = ctx.clone();
        let page_keys = combo_row(
            "翻页键",
            &page_key_labels,
            move |index| {
                ctx_keys.set("general", "page_keys", PAGE_KEY_OPTIONS[index as usize]);
            },
            &syncing,
        );
        page_keys.set_subtitle("选「, .」的话，组句中敲逗号句号是翻页而不是上屏加标点");
        candidates.add(&page_keys);

        let translation = ModifierRow::new(ctx, "shortcut.translation", "第一条译词键", &syncing);
        candidates.add(translation.row());

        let translation_second =
            ModifierRow::new(ctx, "shortcut.translation_second", "第二条译词键", &syncing);
        candidates.add(translation_second.row());

        let translation_space =
            ModifierRow::new(ctx, "shortcut.translation_space", "空格译词键", &syncing);
        candidates.add(translation_space.row());

        let delete_candidate =
            ModifierRow::new(ctx, "shortcut.delete_candidate", "删除候选键", &syncing);
        candidates.add(delete_candidate.row());
        page.add(&candidates);

        let modes = adw::PreferencesGroup::new();
        modes.set_title("模式前缀");
        modes.set_description(Some(
            "只能是 v / u / i 之一且互不相同（这三个字母不是任何拼音音节的开头）",
        ));

        let mode_labels: Vec<String> = MODE_KEY_CANDIDATES
            .iter()
            .map(|key| key.to_string())
            .collect();
        let mode_labels: Vec<&str> = mode_labels.iter().map(String::as_str).collect();
        let ctx_expression = ctx.clone();
        let expression = combo_row(
            "表达式模式键",
            &mode_labels,
            move |index| {
                ctx_expression.set(
                    "shortcut",
                    "expression",
                    MODE_KEY_CANDIDATES[index as usize].to_string(),
                );
            },
            &syncing,
        );
        expression.set_subtitle("v1+2 出 3，v123 出中文数字");
        modes.add(&expression);

        let ctx_question = ctx.clone();
        let question = combo_row(
            "问字模式键",
            &mode_labels,
            move |index| {
                ctx_question.set(
                    "shortcut",
                    "question",
                    MODE_KEY_CANDIDATES[index as usize].to_string(),
                );
            },
            &syncing,
        );
        question.set_subtitle("usangemu 问「三个木」，u4e00 出码点对应的字符");
        modes.add(&question);

        let ctx_mark = ctx.clone();
        let question_mark = switch_row(
            "问号进问字",
            "没在组句时敲 ? 也进问字模式（后面跟字母才是问题，跟别的键还原成问号）",
            move |on| ctx_mark.set("shortcut", "question_mark", on),
            &syncing,
        );
        modes.add(&question_mark);
        page.add(&modes);

        let defaults_group = adw::PreferencesGroup::new();
        let restore_row = adw::ActionRow::new();
        restore_row.set_title("恢复本页默认");
        restore_row.set_subtitle("只改草稿，点「应用」后才保存");
        let restore = gtk::Button::with_label("恢复默认");
        restore.set_valign(gtk::Align::Center);
        restore_row.add_suffix(&restore);
        restore_row.set_activatable_widget(Some(&restore));
        defaults_group.add(&restore_row);
        page.add(&defaults_group);

        let fields = vec![
            ("general.page_keys", page_keys.clone().upcast()),
            ("shortcut.expression", expression.clone().upcast()),
            ("shortcut.question", question.clone().upcast()),
            ("shortcut.question_mark", question_mark.clone().upcast()),
        ];
        let this = Rc::new(Self {
            page,
            ctx: ctx.clone(),
            syncing,
            switch_checks,
            page_keys,
            translation,
            translation_second,
            translation_space,
            delete_candidate,
            expression,
            question,
            question_mark,
            fields,
        });
        let weak = Rc::downgrade(&this);
        restore.connect_clicked(move |_| {
            if let Some(this) = weak.upgrade() {
                this.restore_defaults();
            }
        });
        this
    }

    /// 「恢复本页默认」：把本页字段的缺省值写进草稿并回填控件；点「应用」后才落盘。
    fn restore_defaults(&self) {
        let defaults = ShortcutConfig::default();
        let ctx = &self.ctx;
        ctx.set("general", "page_keys", PAGE_KEY_OPTIONS[0]);
        ctx.set_array(
            "shortcut",
            "switch_mode",
            &defaults.switch_mode.config_values(),
        );
        ctx.set("shortcut", "translation", defaults.translation.key());
        ctx.set(
            "shortcut",
            "translation_second",
            defaults.translation_second.key(),
        );
        ctx.set(
            "shortcut",
            "translation_space",
            defaults.translation_space.key(),
        );
        ctx.set(
            "shortcut",
            "delete_candidate",
            defaults.delete_candidate.key(),
        );
        ctx.set(
            "shortcut",
            "expression",
            defaults.mode.expression.to_string(),
        );
        ctx.set("shortcut", "question", defaults.mode.question.to_string());
        ctx.set("shortcut", "question_mark", defaults.mode.question_mark);
        self.fill(&defaults, PAGE_KEY_OPTIONS[0]);
    }

    /// 用一份快捷键配置回填全部控件（初次 / 撤销 / 恢复默认共用）。
    fn fill(&self, shortcut: &ShortcutConfig, page_keys: &str) {
        self.syncing.set(true);
        for (key, check) in SwitchKey::ALL.iter().zip(&self.switch_checks) {
            check.set_active(shortcut.switch_mode.contains(*key));
        }
        self.page_keys.set_selected(
            PAGE_KEY_OPTIONS
                .iter()
                .position(|option| *option == page_keys)
                .unwrap_or_default() as u32,
        );
        self.translation.reset(shortcut.translation);
        self.translation_second.reset(shortcut.translation_second);
        self.translation_space.reset(shortcut.translation_space);
        self.delete_candidate.reset(shortcut.delete_candidate);
        self.expression.set_selected(
            MODE_KEY_CANDIDATES
                .iter()
                .position(|key| *key == shortcut.mode.expression)
                .unwrap_or_default() as u32,
        );
        self.question.set_selected(
            MODE_KEY_CANDIDATES
                .iter()
                .position(|key| *key == shortcut.mode.question)
                .unwrap_or_default() as u32,
        );
        self.question_mark.set_active(shortcut.mode.question_mark);
        self.syncing.set(false);
    }

    fn modifier_rows(&self) -> [&ModifierRow; 4] {
        [
            &self.translation,
            &self.translation_second,
            &self.translation_space,
            &self.delete_candidate,
        ]
    }
}

impl SettingsPage for ShortcutsPage {
    fn title(&self) -> &'static str {
        "快捷键"
    }

    fn widget(&self) -> &adw::PreferencesPage {
        &self.page
    }

    fn refresh(&self, config: &Config) {
        self.fill(&config.shortcut, &config.general.page_keys);
    }

    fn clear_errors(&self) {
        for (_, row) in &self.fields {
            clear_row_error(row);
        }
    }

    fn mark_error(&self, field: &str, message: &str) -> bool {
        for row in self.modifier_rows() {
            if row.field() == field {
                mark_row_error(row.row(), message);
                return true;
            }
        }
        let Some((_, row)) = self.fields.iter().find(|(path, _)| *path == field) else {
            return false;
        };
        mark_row_error(row, message);
        true
    }

    fn has_invalid_input(&self) -> bool {
        self.modifier_rows().iter().any(|row| row.invalid())
    }
}
