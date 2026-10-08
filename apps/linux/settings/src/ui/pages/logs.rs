//! 日志页：输入日志开关与文件位置、运行日志级别与目录。
//! 「清空输入日志」属于数据维护（P5），本阶段只做禁用态展示。

use adw::prelude::*;
use qingjian_platform::dirs::linux;
use qingjian_platform::{Config, LogLevel};

use super::{
    PageContext, SettingsPage, SyncGuard, clear_row_error, combo_row, mark_row_error, switch_row,
};

pub struct LogsPage {
    page: adw::PreferencesPage,

    syncing: SyncGuard,

    input_log: adw::SwitchRow,

    log_level: adw::ComboRow,

    /// 字段路径 → 行，校验失败时标注用。
    fields: Vec<(&'static str, adw::PreferencesRow)>,
}

impl LogsPage {
    pub fn new(ctx: &PageContext) -> Self {
        let page = adw::PreferencesPage::new();
        let syncing = SyncGuard::default();

        let input = adw::PreferencesGroup::new();
        input.set_title("输入日志");
        let ctx_input = ctx.clone();
        let input_log = switch_row(
            "记录输入日志",
            "每次上屏记一行（敲的键、看到的候选、选了什么），只写在这台电脑上，不上传",
            move |on| ctx_input.set("general", "input_log", on),
            &syncing,
        );
        input.add(&input_log);
        let file = adw::ActionRow::new();
        file.set_title("日志文件");
        file.set_subtitle(
            &linux::user_dir()
                .join("input-log.jsonl")
                .display()
                .to_string(),
        );
        input.add(&file);
        let clear_row = adw::ActionRow::new();
        clear_row.set_title("清空输入日志");
        let clear = gtk::Button::with_label("清空");
        clear.set_valign(gtk::Align::Center);
        clear.set_sensitive(false);
        clear.set_tooltip_text(Some("数据维护在后续版本提供"));
        clear_row.add_suffix(&clear);
        clear_row.set_sensitive(false);
        clear_row.set_tooltip_text(Some("数据维护在后续版本提供"));
        input.add(&clear_row);
        page.add(&input);

        let runtime = adw::PreferencesGroup::new();
        runtime.set_title("运行日志");
        let level_labels: Vec<&str> = LogLevel::ALL.iter().map(|level| level.label()).collect();
        let ctx_level = ctx.clone();
        let log_level = combo_row(
            "日志级别",
            &level_labels,
            move |index| {
                ctx_level.set("general", "log_level", LogLevel::ALL[index as usize].key());
            },
            &syncing,
        );
        log_level.set_subtitle("详细（debug）会记录敲的拼音与上屏的文字，只在配合排查问题时开");
        runtime.add(&log_level);

        let dir_row = adw::ActionRow::new();
        dir_row.set_title("日志目录");
        let log_dir = linux::log_dir();
        dir_row.set_subtitle(&log_dir.display().to_string());
        let open = gtk::Button::with_label("打开");
        open.set_valign(gtk::Align::Center);
        open.connect_clicked(move |_| open_log_dir(&log_dir));
        dir_row.add_suffix(&open);
        dir_row.set_activatable_widget(Some(&open));
        runtime.add(&dir_row);
        page.add(&runtime);

        let fields = vec![
            ("general.input_log", input_log.clone().upcast()),
            ("general.log_level", log_level.clone().upcast()),
        ];
        Self {
            page,
            syncing,
            input_log,
            log_level,
            fields,
        }
    }
}

/// 用系统默认文件管理器打开日志目录；目录还没建就先建（`dirs::linux::log_dir` 不负责创建）。
fn open_log_dir(dir: &std::path::Path) {
    if let Err(error) = std::fs::create_dir_all(dir) {
        tracing::warn!(%error, "创建日志目录失败");
        return;
    }
    let launcher = gtk::FileLauncher::new(Some(&gtk::gio::File::for_path(dir)));
    launcher.launch(
        None::<&gtk::Window>,
        gtk::gio::Cancellable::NONE,
        |result| {
            if let Err(error) = result {
                tracing::warn!(%error, "打开日志目录失败");
            }
        },
    );
}

impl SettingsPage for LogsPage {
    fn title(&self) -> &'static str {
        "日志"
    }

    fn widget(&self) -> &adw::PreferencesPage {
        &self.page
    }

    fn refresh(&self, config: &Config) {
        self.syncing.set(true);
        self.input_log.set_active(config.general.input_log);
        self.log_level.set_selected(
            LogLevel::ALL
                .iter()
                .position(|level| *level == config.general.log_level)
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
