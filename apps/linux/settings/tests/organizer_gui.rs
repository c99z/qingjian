//! 原生 GTK 夜间整理开关、时间和领域提示的草稿、保存、撤销与重新打开。

use adw::prelude::{
    ApplicationExt, ButtonExt, Cast, EditableExt, GtkApplicationExt, GtkWindowExt,
    PreferencesRowExt, WidgetExt,
};
use gtk::gio;
use qingjian_linux_settings::{state::DraftStore, ui::window};
use qingjian_platform::Config;

#[path = "support/gui.rs"]
#[allow(dead_code)]
mod support;

#[test]
fn organizer_settings_save_revert_and_reopen() {
    if std::env::var_os("QINGJIAN_GUI_TEST").is_none() {
        return;
    }
    adw::init().unwrap();
    let app = adw::Application::builder()
        .application_id("im.qingjian.Settings.OrganizerTest")
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gio::Cancellable>).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(&path, "[model]\nenabled=false\n").unwrap();
    let settings = window::build(&app, DraftStore::load(&path).unwrap());
    settings.present();
    let window = app.windows().remove(0);
    support::wait_for(|| window.is_mapped());
    let widgets = support::descendants(&window.clone().upcast());
    let toggle = widgets
        .iter()
        .filter_map(|w| w.clone().downcast::<adw::SwitchRow>().ok())
        .find(|r| r.title() == "夜间自动整理个人词库")
        .unwrap();
    let hour = widgets
        .iter()
        .filter_map(|w| w.clone().downcast::<adw::SpinRow>().ok())
        .find(|r| r.title() == "整理时间：小时（0–23）")
        .unwrap();
    let hint = widgets
        .iter()
        .filter_map(|w| w.clone().downcast::<adw::EntryRow>().ok())
        .find(|r| r.title() == "专业领域提示（可选，最多 120 字）")
        .unwrap();
    let revert = widgets
        .iter()
        .filter_map(|w| w.clone().downcast::<gtk::Button>().ok())
        .find(|b| b.label().as_deref() == Some("撤销修改"))
        .unwrap();
    assert!(!toggle.is_active());
    assert_eq!(hour.value(), 3.0);
    toggle.set_active(true);
    hour.set_value(2.0);
    hint.set_text("焊接、机器人仿真");
    assert!(!Config::load(&path).unwrap().organizer.enabled);
    revert.emit_clicked();
    assert!(!toggle.is_active());
    assert_eq!(hour.value(), 3.0);
    assert!(hint.text().is_empty());
    toggle.set_active(true);
    hour.set_value(2.0);
    hint.set_text("焊接、机器人仿真");
    window.activate_action("win.apply", None).unwrap();
    support::wait_for(|| Config::load(&path).unwrap().organizer.enabled);
    let saved = Config::load(&path).unwrap();
    assert_eq!(saved.organizer.hour, 2);
    assert_eq!(saved.organizer.domain_hint, "焊接、机器人仿真");
    assert!(!saved.predict.enabled);
    window.destroy();
    let settings = window::build(&app, DraftStore::load(&path).unwrap());
    settings.present();
    let window = app.windows().remove(0);
    support::wait_for(|| window.is_mapped());
    let widgets = support::descendants(&window.clone().upcast());
    assert!(
        widgets
            .iter()
            .filter_map(|w| w.clone().downcast::<adw::SwitchRow>().ok())
            .find(|r| r.title() == "夜间自动整理个人词库")
            .unwrap()
            .is_active()
    );
    window.destroy();
}
