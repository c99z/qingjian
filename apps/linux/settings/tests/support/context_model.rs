//! 真实 GTK 语境开关的默认值、撤销、保存与父开关依赖。

use std::path::Path;

use adw::prelude::{ButtonExt, Cast, PreferencesRowExt, WidgetExt};
use qingjian_platform::Config;

use crate::support::wait_for;

pub fn verify(window: &gtk::Window, stack: &gtk::Stack, widgets: &[gtk::Widget], path: &Path) {
    let row = |title: &str| {
        widgets
            .iter()
            .filter_map(|w| w.clone().downcast::<adw::SwitchRow>().ok())
            .find(|r| r.title() == title)
            .unwrap()
    };
    let context = row("结合最近输入优化候选（实验性）");
    let enabled = row("启用本地模型");
    let revert = widgets
        .iter()
        .filter_map(|w| w.clone().downcast::<gtk::Button>().ok())
        .find(|b| b.label().as_deref() == Some("撤销修改"))
        .unwrap();
    assert!(!context.is_active());
    context.set_active(true);
    assert!(!Config::load(path).unwrap().model.context_ranking);
    revert.emit_clicked();
    assert!(!context.is_active());
    context.set_active(true);
    window.activate_action("win.apply", None).unwrap();
    wait_for(|| stack.is_sensitive() && Config::load(path).unwrap().model.context_ranking);
    enabled.set_active(false);
    assert!(!context.is_sensitive());
    revert.emit_clicked();
    assert!(context.is_sensitive());
    assert!(context.is_active());
    context.set_active(false);
    window.activate_action("win.apply", None).unwrap();
    wait_for(|| stack.is_sensitive() && !Config::load(path).unwrap().model.context_ranking);
}
