//! 可选的真实 GTK 检查：草稿、撤销、后台保存以及外观页渲染；无显示环境时跳过。

use std::time::{Duration, Instant};

use adw::prelude::*;
use gtk::gio;
#[path = "support/gui.rs"]
mod support;
use qingjian_linux_settings::state::DraftStore;
use qingjian_linux_settings::ui::window;
use support::{capture, descendants, pump, wait_for};
#[path = "support/layout.rs"]
mod layout;

#[test]
fn native_window_edits_reverts_saves_and_renders() {
    if std::env::var_os("QINGJIAN_GUI_TEST").is_none() {
        return;
    }
    adw::init().unwrap();
    let app = adw::Application::builder()
        .application_id("im.qingjian.Settings.Test")
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gio::Cancellable>).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let original = "# GUI 草稿检查\n[general]\npage_size = 9 # 保留注释\n";
    std::fs::write(&path, original).unwrap();
    let settings = window::build(&app, DraftStore::load(&path).unwrap());
    settings.present();
    let window = app.windows().remove(0);
    wait_for(|| window.is_mapped());
    let widgets = descendants(&window.clone().upcast());
    let stack = widgets
        .iter()
        .find_map(|w| w.clone().downcast::<gtk::Stack>().ok())
        .unwrap();
    let spin = widgets
        .iter()
        .filter_map(|w| w.clone().downcast::<adw::SpinRow>().ok())
        .find(|row| row.title() == "每页候选数")
        .unwrap();
    spin.set_value(5.0);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
    let revert = widgets
        .iter()
        .filter_map(|w| w.clone().downcast::<gtk::Button>().ok())
        .find(|b| b.label().as_deref() == Some("撤销修改"))
        .unwrap();
    revert.emit_clicked();
    assert_eq!(spin.value(), 9.0);
    spin.set_value(6.0);
    window.activate_action("win.apply", None).unwrap();
    wait_for(|| {
        stack.is_sensitive()
            && qingjian_platform::Config::load(&path)
                .unwrap()
                .general
                .page_size
                == 6
    });
    assert!(
        std::fs::read_to_string(&path)
            .unwrap()
            .contains("# 保留注释")
    );
    let switch_check = |title: &str| {
        let row = widgets
            .iter()
            .filter_map(|w| w.clone().downcast::<adw::ActionRow>().ok())
            .find(|row| row.title() == title)
            .unwrap();
        descendants(&row.upcast())
            .into_iter()
            .find_map(|w| w.downcast::<gtk::CheckButton>().ok())
            .unwrap()
    };
    switch_check("单击 Shift").set_active(false);
    switch_check("Ctrl + Alt + Space").set_active(true);
    window.activate_action("win.apply", None).unwrap();
    wait_for(|| {
        stack.is_sensitive()
            && qingjian_platform::Config::load(&path)
                .unwrap()
                .shortcut
                .switch_mode
                .config_values()
                == ["ctrl+alt+space"]
    });
    let sidebar = widgets
        .iter()
        .filter_map(|w| w.clone().downcast::<gtk::ListBox>().ok())
        .find(|list| list.has_css_class("navigation-sidebar"))
        .unwrap();
    sidebar.select_row(sidebar.row_at_index(1).as_ref());
    wait_for(|| stack.visible_child_name().as_deref() == Some("1"));
    layout::verify(&window, &stack, &widgets, &path);
    let split = widgets
        .iter()
        .find_map(|w| w.clone().downcast::<adw::NavigationSplitView>().ok())
        .unwrap();
    for (scheme, name) in [
        (adw::ColorScheme::ForceLight, "light"),
        (adw::ColorScheme::ForceDark, "dark"),
    ] {
        adw::StyleManager::default().set_color_scheme(scheme);
        for width in [1000, 720] {
            window.set_default_size(width, 720);
            let until = Instant::now() + Duration::from_millis(150);
            while Instant::now() < until {
                pump();
            }
            assert_eq!(split.is_collapsed(), width <= 800);
            if let Some(prefix) = std::env::var_os("QINGJIAN_GUI_CAPTURE") {
                capture(
                    &window,
                    &format!("{}-{name}-{width}.png", prefix.to_string_lossy()),
                );
            }
        }
    }
    revert.emit_clicked();
    sidebar.select_row(sidebar.row_at_index(5).as_ref());
    wait_for(|| stack.visible_child_name().as_deref() == Some("5"));
    let stats_source = descendants(&window.clone().upcast())
        .into_iter()
        .filter_map(|w| w.downcast::<adw::ActionRow>().ok())
        .find(|r| r.title() == "数据来源")
        .unwrap();
    wait_for(|| {
        stats_source
            .subtitle()
            .as_deref()
            .is_some_and(|s| s != "读取中…")
    });
    assert!(stats_source.subtitle().unwrap().contains("服务未运行"));
    // 新页面真实控件编辑与密钥落盘，不使用真实密钥和外网。
    sidebar.select_row(sidebar.row_at_index(8).as_ref());
    wait_for(|| stack.visible_child_name().as_deref() == Some("8"));
    let cloud_widgets = descendants(&window.clone().upcast());
    let url = cloud_widgets
        .iter()
        .filter_map(|w| w.clone().downcast::<adw::EntryRow>().ok())
        .find(|r| r.title() == "API URL")
        .unwrap();
    let model = cloud_widgets
        .iter()
        .filter_map(|w| w.clone().downcast::<adw::EntryRow>().ok())
        .find(|r| r.title() == "模型名称")
        .unwrap();
    let key = cloud_widgets
        .iter()
        .filter_map(|w| w.clone().downcast::<adw::PasswordEntryRow>().ok())
        .find(|r| r.title().starts_with("API Key"))
        .unwrap();
    url.set_text("http://127.0.0.1:12345/v1");
    model.set_text("test-model");
    key.set_text("gui-fake-key");
    window.activate_action("win.apply", None).unwrap();
    wait_for(|| stack.is_sensitive() && path.with_file_name(".env").exists());
    assert!(
        std::fs::read_to_string(path.with_file_name(".env"))
            .unwrap()
            .contains("gui-fake-key")
    );
    assert!(
        !std::fs::read_to_string(&path)
            .unwrap()
            .contains("gui-fake-key")
    );
    for index in [6, 7, 8, 9, 10] {
        sidebar.select_row(sidebar.row_at_index(index).as_ref());
        wait_for(|| stack.visible_child_name().as_deref() == Some(index.to_string().as_str()));
        if let Some(prefix) = std::env::var_os("QINGJIAN_GUI_CAPTURE") {
            let until = Instant::now() + Duration::from_millis(100);
            while Instant::now() < until {
                pump();
            }
            capture(
                &window,
                &format!("{}-page-{index}.png", prefix.to_string_lossy()),
            );
        }
    }
    let account_fields = descendants(&window.clone().upcast());
    assert!(
        account_fields
            .iter()
            .filter_map(|w| w.clone().downcast::<adw::EntryRow>().ok())
            .any(|r| r.title() == "账号服务 URL")
    );
    assert!(
        account_fields
            .iter()
            .filter_map(|w| w.clone().downcast::<gtk::Button>().ok())
            .any(|b| b.label().as_deref() == Some("登录"))
    );
    window.destroy();
}
