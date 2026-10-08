//! 各设置页：通用、快捷键、本地模型、日志。所有页面共享同一份草稿（[`DraftStore`]），
//! 底栏的「撤销修改 / 应用」对全部页面一起生效。

pub mod appearance;
pub mod general;
pub mod logs;
pub mod model;
pub mod shortcuts;
pub mod statistics;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use adw::prelude::*;
use qingjian_platform::Config;

use crate::adapters::admin::AdminClient;
use crate::state::DraftStore;

/// 页面与窗口共享的上下文。
#[derive(Clone)]
pub struct PageContext {
    pub store: Rc<RefCell<DraftStore>>,

    /// 草稿变了之后让窗口刷新底栏按钮与横幅。
    pub notify: Rc<dyn Fn()>,

    pub admin: Arc<AdminClient>,
}

impl PageContext {
    /// 控件回调里记一处字段修改并通知窗口。
    pub fn set(&self, section: &str, key: &str, value: impl Into<toml_edit::Value>) {
        self.store.borrow_mut().set(section, key, value);
        (self.notify)();
    }

    /// 记一个字符串数组字段（如 `[shortcut] switch_mode`）。
    pub fn set_array(&self, section: &str, key: &str, values: &[&str]) {
        let mut array = toml_edit::Array::new();
        for value in values {
            array.push(*value);
        }
        self.set(section, key, array);
    }
}

/// 一个设置页。
pub trait SettingsPage {
    /// 侧栏与内容页标题。
    fn title(&self) -> &'static str;

    fn widget(&self) -> &adw::PreferencesPage;

    /// 从配置回填全部控件（初次、撤销、重新载入、保存成功后）。
    fn refresh(&self, config: &Config);

    fn clear_errors(&self);

    /// 认领并标注一个字段错误；认领返回 true（窗口据此跳到对应页面）。
    fn mark_error(&self, field: &str, message: &str) -> bool;

    /// 有没有没进草稿的非法输入（如修饰键输入框里的非法写法）；有就挡住「应用」。
    fn has_invalid_input(&self) -> bool {
        false
    }

    /// 一次保存成功后调用：页面可以重新拉取外部状态（如 Fcitx5 侧的实际值）。
    fn on_saved(&self) {}
}

/// 页面回填控件时屏蔽「值变化」回调的旗标，避免回填又被当成用户编辑写回草稿。
pub type SyncGuard = Rc<Cell<bool>>;

/// 给枚举字段建 ComboRow；`on_select` 收到选中下标。
pub fn combo_row(
    title: &str,
    labels: &[&str],
    on_select: impl Fn(u32) + 'static,
    syncing: &SyncGuard,
) -> adw::ComboRow {
    let row = adw::ComboRow::new();
    row.set_title(title);
    row.set_model(Some(&gtk::StringList::new(labels)));
    let syncing = syncing.clone();
    row.connect_selected_notify(move |row| {
        if !syncing.get() {
            on_select(row.selected());
        }
    });
    row
}

/// 布尔字段的 SwitchRow。
pub fn switch_row(
    title: &str,
    subtitle: &str,
    on_toggle: impl Fn(bool) + 'static,
    syncing: &SyncGuard,
) -> adw::SwitchRow {
    let row = adw::SwitchRow::new();
    row.set_title(title);
    if !subtitle.is_empty() {
        row.set_subtitle(subtitle);
    }
    let syncing = syncing.clone();
    row.connect_active_notify(move |row| {
        if !syncing.get() {
            on_toggle(row.is_active());
        }
    });
    row
}

/// 标注字段错误：error 样式 + tooltip 说明。
pub fn mark_row_error(row: &impl IsA<gtk::Widget>, message: &str) {
    let widget = row.upcast_ref::<gtk::Widget>();
    widget.add_css_class("error");
    widget.set_tooltip_text(Some(message));
}

pub fn clear_row_error(row: &impl IsA<gtk::Widget>) {
    let widget = row.upcast_ref::<gtk::Widget>();
    widget.remove_css_class("error");
    widget.set_tooltip_text(None);
}
