//! 中文策略的真实控件保存、撤销与关联开关敏感性。

use std::path::Path;

use adw::prelude::*;
use qingjian_core::ChineseInputStyle;
use qingjian_platform::Config;

use crate::support::wait_for;

pub fn verify(window: &gtk::Window, stack: &gtk::Stack, widgets: &[gtk::Widget], path: &Path) {
    let style = widgets
        .iter()
        .filter_map(|w| w.clone().downcast::<adw::ComboRow>().ok())
        .find(|r| r.title() == "中文输入策略")
        .unwrap();
    let first = widgets
        .iter()
        .filter_map(|w| w.clone().downcast::<adw::SwitchRow>().ok())
        .find(|r| r.title() == "中文优先")
        .unwrap();
    let revert = widgets
        .iter()
        .filter_map(|w| w.clone().downcast::<gtk::Button>().ok())
        .find(|b| b.label().as_deref() == Some("撤销修改"))
        .unwrap();
    assert_eq!(style.selected(), 1);
    style.set_selected(0);
    assert!(!first.is_sensitive());
    assert_eq!(
        Config::load(path).unwrap().general.chinese_input_style,
        ChineseInputStyle::Mixed
    );
    revert.emit_clicked();
    assert_eq!(style.selected(), 1);
    assert!(first.is_sensitive());
    for (index, expected) in [
        (0, ChineseInputStyle::Chinese),
        (1, ChineseInputStyle::Mixed),
    ] {
        style.set_selected(index);
        window.activate_action("win.apply", None).unwrap();
        wait_for(|| {
            stack.is_sensitive()
                && Config::load(path).unwrap().general.chinese_input_style == expected
        });
        assert_eq!(first.is_sensitive(), expected.mixed());
    }
}
