//! 应用装配：`AdwApplication` 与单实例行为。
//!
//! 单实例：GTK 应用默认就是单实例（不设 `NON_UNIQUE`）——重复启动时系统把 activate 发给
//! 已在跑的进程，这里把已有窗口提到前台，不会再开一个窗口（对应验收「重复启动只打开一个设置窗口」）。

use adw::prelude::*;
use gtk::glib;
use qingjian_platform::dirs::linux;

use crate::state::DraftStore;

use super::window;

/// 桌面入口与 D-Bus 激活用的应用 ID。
pub const APP_ID: &str = "im.qingjian.Settings";

pub fn run() -> glib::ExitCode {
    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_activate(|app| {
        // 单实例：已有窗口就提到前台
        if let Some(existing) = app.active_window() {
            existing.present();
            return;
        }
        match DraftStore::load(&linux::config_path()) {
            Ok(store) => window::build(app, store).present(),
            Err(error) => {
                // 连读都读不了（权限等）：没有可编辑的基准，只能报错退出
                tracing::error!(%error, "读取配置文件失败");
                let dialog = adw::AlertDialog::builder()
                    .heading("无法读取配置文件")
                    .body(format!("{}：{error}", linux::config_path().display()))
                    .build();
                dialog.add_responses(&[("quit", "退出")]);
                dialog.set_close_response("quit");
                let app = app.clone();
                dialog.connect_response(None, move |_, _| app.quit());
                dialog.present(None::<&gtk::Widget>);
            }
        }
    });
    app.set_accels_for_action("win.apply", &["<Ctrl>s"]);
    app.run()
}
