//! GTK 测试中的控件遍历与事件循环。
use adw::prelude::*;
use gtk::glib;
use std::time::{Duration, Instant};

pub fn descendants(widget: &gtk::Widget) -> Vec<gtk::Widget> {
    let mut all = vec![widget.clone()];
    let mut child = widget.first_child();
    while let Some(current) = child {
        all.extend(descendants(&current));
        child = current.next_sibling();
    }
    all
}

pub fn pump() {
    let context = glib::MainContext::default();
    while context.pending() {
        context.iteration(false);
    }
    std::thread::sleep(Duration::from_millis(10));
}

pub fn wait_for(predicate: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while !predicate() {
        assert!(Instant::now() < deadline, "GTK 状态没有按时更新");
        pump();
    }
}

/// 样式切换后等到窗口完成真实绘制，不能把尚未绘制的一帧当作截图。
pub fn capture(window: &gtk::Window, path: &str) {
    window.queue_draw();
    let paintable = gtk::WidgetPaintable::new(Some(window));
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        pump();
        let snapshot = gtk::Snapshot::new();
        paintable.snapshot(&snapshot, window.width() as f64, window.height() as f64);
        if let Some(node) = snapshot.to_node() {
            window
                .renderer()
                .unwrap()
                .render_texture(&node, None)
                .save_to_png(path)
                .unwrap();
            return;
        }
        assert!(
            Instant::now() < deadline,
            "窗口没有完成绘制：mapped={} visible={} size={}x{}",
            window.is_mapped(),
            window.is_visible(),
            window.width(),
            window.height()
        );
    }
}

/// 账号页仍可访问登录字段。
pub fn verify_account(window: &gtk::Window) {
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
}
