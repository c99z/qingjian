//! 实际设置窗口的排列草稿、预览与保存回归。
use adw::prelude::*;
use std::path::Path;

use crate::support::{descendants, pump, wait_for};

pub fn verify(window: &gtk::Window, stack: &gtk::Stack, widgets: &[gtk::Widget], path: &Path) {
    // 等待外观 D-Bus 探测完成，然后以 GTK 实际渲染输出检查布局，无需桌面焦点。
    let status = descendants(&window.clone().upcast())
        .into_iter()
        .filter_map(|w| w.downcast::<adw::ActionRow>().ok())
        .find(|r| r.title() == "Fcitx5")
        .unwrap();
    wait_for(|| status.subtitle().as_deref() != Some("检测中…"));
    let layout = widgets
        .iter()
        .filter_map(|w| w.clone().downcast::<adw::ComboRow>().ok())
        .find(|row| row.title() == "候选排列")
        .unwrap();
    let preview_rows: Vec<gtk::Box> = widgets
        .iter()
        .filter(|w| w.has_css_class("qj-candidate-row"))
        .map(|w| w.clone().downcast::<gtk::Box>().unwrap())
        .collect();
    assert_eq!(preview_rows.len(), 5);
    layout.set_selected(1);
    pump();
    for row in &preview_rows {
        assert_eq!(row.orientation(), gtk::Orientation::Vertical);
        assert!(row.last_child().unwrap().is_visible());
    }
    assert!(
        !std::fs::read_to_string(path)
            .unwrap()
            .contains("horizontal")
    );
    window.activate_action("win.apply", None).unwrap();
    wait_for(|| {
        stack.is_sensitive()
            && qingjian_platform::Config::load(path)
                .unwrap()
                .general
                .layout
                == qingjian_platform::LayoutMode::Horizontal
    });
    layout.set_selected(0);
    pump();
    for row in &preview_rows {
        assert_eq!(row.orientation(), gtk::Orientation::Horizontal);
    }
    window.activate_action("win.apply", None).unwrap();
    wait_for(|| {
        stack.is_sensitive()
            && qingjian_platform::Config::load(path)
                .unwrap()
                .general
                .layout
                == qingjian_platform::LayoutMode::Vertical
    });
    let expander = descendants(&window.clone().upcast())
        .into_iter()
        .find_map(|w| w.downcast::<adw::ExpanderRow>().ok())
        .unwrap();
    assert!(expander.shows_enable_switch());
    expander.set_enable_expansion(true);
    expander.set_expanded(true);
}
