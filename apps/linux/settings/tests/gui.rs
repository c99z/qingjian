//! 可选的真实 GTK 检查：草稿、撤销、后台保存以及外观页渲染；无显示环境时跳过。

use std::time::{Duration, Instant};

use adw::prelude::*;
use gtk::{gio, glib};
use qingjian_linux_settings::state::DraftStore;
use qingjian_linux_settings::ui::window;

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
    let sidebar = widgets
        .iter()
        .filter_map(|w| w.clone().downcast::<gtk::ListBox>().ok())
        .find(|list| list.has_css_class("navigation-sidebar"))
        .unwrap();
    sidebar.select_row(sidebar.row_at_index(1).as_ref());
    wait_for(|| stack.visible_child_name().as_deref() == Some("1"));
    // 等待外观 D-Bus 探测完成，然后以 GTK 实际渲染输出检查布局，无需桌面焦点。
    let status = descendants(&window.clone().upcast())
        .into_iter()
        .filter_map(|w| w.downcast::<adw::ActionRow>().ok())
        .find(|r| r.title() == "Fcitx5")
        .unwrap();
    wait_for(|| status.subtitle().as_deref() != Some("检测中…"));
    let expander = descendants(&window.clone().upcast())
        .into_iter()
        .find_map(|w| w.downcast::<adw::ExpanderRow>().ok())
        .unwrap();
    assert!(expander.shows_enable_switch());
    expander.set_enable_expansion(true);
    expander.set_expanded(true);
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
                let paintable = gtk::WidgetPaintable::new(Some(&window));
                let snapshot = gtk::Snapshot::new();
                paintable.snapshot(&snapshot, window.width() as f64, window.height() as f64);
                let node = snapshot.to_node().unwrap();
                let texture = window.renderer().unwrap().render_texture(&node, None);
                texture
                    .save_to_png(format!("{}-{name}-{width}.png", prefix.to_string_lossy()))
                    .unwrap();
            }
        }
    }
    revert.emit_clicked();
    window.destroy();
}

fn descendants(widget: &gtk::Widget) -> Vec<gtk::Widget> {
    let mut all = vec![widget.clone()];
    let mut child = widget.first_child();
    while let Some(current) = child {
        all.extend(descendants(&current));
        child = current.next_sibling();
    }
    all
}

fn pump() {
    let context = glib::MainContext::default();
    while context.pending() {
        context.iteration(false);
    }
    std::thread::sleep(Duration::from_millis(10));
}

fn wait_for(predicate: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while !predicate() {
        assert!(Instant::now() < deadline, "GTK 状态没有按时更新");
        pump();
    }
}
