//! 单独验证语境设置的真实 GTK 控件，不将保存检查与窗口管理器缩放混为一项。

use adw::prelude::{
    ApplicationExt, Cast, GtkApplicationExt, GtkWindowExt, PreferencesRowExt, WidgetExt,
};
use gtk::gio;
use qingjian_linux_settings::{state::DraftStore, ui::window};

#[path = "support/context_model.rs"]
mod context_model;
#[path = "support/gui.rs"]
#[allow(dead_code)]
mod support;

#[test]
fn native_context_model_controls_save_revert_and_reopen() {
    if std::env::var_os("QINGJIAN_GUI_TEST").is_none() {
        return;
    }
    adw::init().unwrap();
    let app = adw::Application::builder()
        .application_id("im.qingjian.Settings.ContextTest")
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gio::Cancellable>).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(&path, "[model]\nenabled = true\n").unwrap();
    let settings = window::build(&app, DraftStore::load(&path).unwrap());
    settings.present();
    let window = app.windows().remove(0);
    support::wait_for(|| window.is_mapped());
    let widgets = support::descendants(&window.clone().upcast());
    let stack = widgets
        .iter()
        .find_map(|w| w.clone().downcast::<gtk::Stack>().ok())
        .unwrap();
    context_model::verify(&window, &stack, &widgets, &path);
    window.destroy();
    let reloaded = DraftStore::load(&path).unwrap();
    let settings = window::build(&app, reloaded);
    settings.present();
    let window = app.windows().remove(0);
    support::wait_for(|| window.is_mapped());
    let reopened = support::descendants(&window.clone().upcast())
        .into_iter()
        .filter_map(|w| w.downcast::<adw::SwitchRow>().ok())
        .find(|row| row.title() == "结合最近输入优化候选（实验性）")
        .unwrap();
    assert!(!reopened.is_active());
    window.destroy();
}
