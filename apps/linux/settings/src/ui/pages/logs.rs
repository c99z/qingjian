//! 日志页：输入日志开关与文件位置、运行日志级别与目录。
//! 清空操作由服务协调，避免旧日志缓冲在截断之后重新写回。

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

    learning: adw::SwitchRow,

    log_level: adw::ComboRow,

    /// 字段路径 → 行，校验失败时标注用。
    fields: Vec<(&'static str, adw::PreferencesRow)>,
}

impl LogsPage {
    pub fn new(ctx: &PageContext) -> Self {
        let page = adw::PreferencesPage::new();
        let syncing = SyncGuard::default();

        let data = adw::PreferencesGroup::new();
        data.set_title("学习与数据");
        let learning_ctx = ctx.clone();
        let learning = switch_row(
            "学习输入习惯",
            "记住选词习惯与用户词；关闭后不再产生新的学习记录",
            move |on| learning_ctx.set("general", "learning", on),
            &syncing,
        );
        data.add(&learning);
        let data_path = adw::ActionRow::new();
        data_path.set_title("学习数据目录");
        data_path.set_subtitle(&linux::user_dir().display().to_string());
        data.add(&data_path);
        page.add(&data);

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
            &linux::log_dir()
                .join("input-log.jsonl")
                .display()
                .to_string(),
        );
        input.add(&file);
        let clear_row = adw::ActionRow::new();
        clear_row.set_title("清空输入日志");
        let clear = gtk::Button::with_label("清空");
        clear.set_valign(gtk::Align::Center);
        clear_row.set_subtitle("立即操作；清空后无法撤销，学习数据和统计保留");
        let admin = ctx.admin.clone();
        let status = clear_row.clone();
        clear.connect_clicked(move |button| {
            let parent = button.root().and_then(|root| root.downcast::<gtk::Window>().ok());
            let dialog = adw::AlertDialog::builder().heading("清空输入日志？")
                .body("将立即清空已记录的输入日志，无法撤销；学习数据和输入统计保留。青简服务必须正在运行。").build();
            dialog.add_responses(&[("cancel", "返回"), ("clear", "清空")]);
            dialog.set_response_appearance("clear", adw::ResponseAppearance::Destructive);
            dialog.set_default_response(Some("cancel"));
            dialog.set_close_response("cancel");
            let admin = admin.clone();
            let status = status.clone();
            let button = button.clone();
            dialog.connect_response(None, move |_, response| {
                if response != "clear" { return; }
                button.set_sensitive(false);
                status.set_subtitle("正在清空…");
                let admin = admin.clone();
                let status = status.clone();
                let button = button.clone();
                gtk::glib::MainContext::default().spawn_local(async move {
                    let result = gtk::gio::spawn_blocking(move || admin.clear_input_log()).await;
                    use qingjian_platform::settings::ComponentStatus;
                    let message = match result {
                        Ok(Ok(ComponentStatus::Applied)) => "输入日志已清空".to_owned(),
                        Ok(Ok(ComponentStatus::Failed(message))) => message,
                        Ok(Ok(_)) => "服务未确认清空，请重新检查".to_owned(),
                        Ok(Err(error)) => error.user_message(),
                        Err(_) => "清空失败：后台任务中断".to_owned(),
                    };
                    status.set_subtitle(&message);
                    button.set_sensitive(true);
                });
            });
            dialog.present(parent.as_ref());
        });
        clear_row.add_suffix(&clear);
        clear_row.set_activatable_widget(Some(&clear));
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
            ("general.learning", learning.clone().upcast()),
            ("general.log_level", log_level.clone().upcast()),
        ];
        Self {
            page,
            syncing,
            input_log,
            learning,
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
        "学习与数据"
    }

    fn widget(&self) -> &adw::PreferencesPage {
        &self.page
    }

    fn refresh(&self, config: &Config) {
        self.syncing.set(true);
        self.input_log.set_active(config.general.input_log);
        self.learning.set_active(config.general.learning);
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
