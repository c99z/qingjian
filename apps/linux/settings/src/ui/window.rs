//! 设置主窗口：左侧分类 + 右侧页面（`AdwNavigationSplitView`，窄窗口自动收起侧栏）、
//! 底部「撤销修改 / 应用」、校验与状态横幅、冲突与关闭确认对话框。
//!
//! 耗时工作（保存、读写配置）走 `gio::spawn_blocking`，结果回主线程更新；GUI 操作留在主线程。

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use adw::prelude::*;
use gtk::{gio, glib};
use qingjian_platform::settings::{ComponentStatus, FieldConflict, FieldError};

use crate::adapters::admin::AdminClient;
use crate::state::{DraftStore, SaveOutcome, save};

use super::pages::appearance::AppearancePage;
use super::pages::general::GeneralPage;
use super::pages::logs::LogsPage;
use super::pages::model::ModelPage;
use super::pages::shortcuts::ShortcutsPage;
use super::pages::{PageContext, SettingsPage};

pub struct SettingsWindow {
    window: adw::ApplicationWindow,

    ctx: PageContext,

    pages: Vec<Rc<dyn SettingsPage>>,

    sidebar: gtk::ListBox,

    stack: gtk::Stack,

    split: adw::NavigationSplitView,

    content_page: adw::NavigationPage,

    banner: adw::Banner,

    toast: adw::ToastOverlay,

    saving: Rc<Cell<bool>>,

    /// 非常驻横幅（保存失败 / 校验不过）；配置语法错误的横幅直接读快照，不经过这里。
    notice: Rc<RefCell<Option<String>>>,
}

/// 建窗口并接好全部信号；`store` 已读过配置快照。
pub fn build(app: &adw::Application, store: DraftStore) -> Rc<SettingsWindow> {
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("青简设置")
        .width_request(360)
        .height_request(320)
        .default_width(1000)
        .default_height(720)
        .build();

    let banner = adw::Banner::new("");
    let split = adw::NavigationSplitView::new();
    split.set_vexpand(true);
    split.set_min_sidebar_width(220.0);
    split.set_max_sidebar_width(320.0);
    let breakpoint = adw::Breakpoint::new(
        adw::BreakpointCondition::parse("max-width: 800px").expect("固定断点合法"),
    );
    breakpoint.add_setter(&split, "collapsed", Some(&true.to_value()));
    window.add_breakpoint(breakpoint);

    let revert = gtk::Button::with_label("撤销修改");
    revert.set_tooltip_text(Some("放弃全部未保存的修改"));
    let apply = gtk::Button::with_label("应用");
    apply.set_tooltip_text(Some("校验并保存全部修改（Ctrl+S）"));
    apply.add_css_class("suggested-action");
    let bottom = gtk::ActionBar::new();
    bottom.pack_start(&revert);
    bottom.pack_end(&apply);

    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root.append(&banner);
    root.append(&split);
    root.append(&bottom);
    let toast = adw::ToastOverlay::new();
    toast.set_child(Some(&root));
    window.set_content(Some(&toast));

    let store = Rc::new(RefCell::new(store));
    let saving = Rc::new(Cell::new(false));
    let notice = Rc::new(RefCell::new(None));

    // 草稿变化通知：刷新底栏按钮与横幅。先建闭包再建页面，页面回调经 PageContext 汇到这里。
    let notify = {
        let store = store.clone();
        let apply = apply.clone();
        let revert = revert.clone();
        let banner = banner.clone();
        let saving = saving.clone();
        let notice = notice.clone();
        Rc::new(move || {
            let dirty = store.borrow().is_dirty();
            let busy = saving.get();
            apply.set_sensitive(dirty && !busy);
            revert.set_sensitive(dirty && !busy);
            update_banner(&store, &banner, &notice);
        }) as Rc<dyn Fn()>
    };

    let ctx = PageContext {
        store,
        notify,
        admin: Arc::new(AdminClient::new()),
    };

    let pages: Vec<Rc<dyn SettingsPage>> = vec![
        Rc::new(GeneralPage::new(&ctx)),
        AppearancePage::new(&ctx),
        ShortcutsPage::new(&ctx),
        Rc::new(ModelPage::new(&ctx)),
        Rc::new(LogsPage::new(&ctx)),
    ];

    let sidebar = gtk::ListBox::new();
    sidebar.add_css_class("navigation-sidebar");
    let stack = gtk::Stack::new();
    for (index, page) in pages.iter().enumerate() {
        let label = gtk::Label::new(Some(page.title()));
        label.set_xalign(0.0);
        sidebar.append(&label);
        stack.add_named(page.widget(), Some(&index.to_string()));
    }

    let content_toolbar = adw::ToolbarView::new();
    content_toolbar.add_top_bar(&adw::HeaderBar::new());
    content_toolbar.set_content(Some(&stack));
    let content_page = adw::NavigationPage::new(&content_toolbar, pages[0].title());
    content_page.set_tag(Some("content"));

    let sidebar_toolbar = adw::ToolbarView::new();
    sidebar_toolbar.add_top_bar(&adw::HeaderBar::new());
    let sidebar_scroll = gtk::ScrolledWindow::new();
    sidebar_scroll.set_child(Some(&sidebar));
    sidebar_toolbar.set_content(Some(&sidebar_scroll));
    let sidebar_page = adw::NavigationPage::new(&sidebar_toolbar, "青简设置");
    sidebar_page.set_tag(Some("sidebar"));

    split.set_sidebar(Some(&sidebar_page));
    split.set_content(Some(&content_page));

    let this = Rc::new(SettingsWindow {
        window: window.clone(),
        ctx,
        pages,
        sidebar: sidebar.clone(),
        stack,
        split,
        content_page,
        banner,
        toast,
        saving,
        notice,
    });

    let weak = Rc::downgrade(&this);
    sidebar.connect_row_selected(move |_, row| {
        if let Some(this) = weak.upgrade() {
            this.on_row_selected(row);
        }
    });

    let weak = Rc::downgrade(&this);
    revert.connect_clicked(move |_| {
        if let Some(this) = weak.upgrade() {
            this.revert_all();
        }
    });

    let weak = Rc::downgrade(&this);
    apply.connect_clicked(move |_| {
        if let Some(this) = weak.upgrade() {
            this.start_save(false);
        }
    });

    // Ctrl+S 保存
    let apply_action = gio::SimpleAction::new("apply", None);
    let weak = Rc::downgrade(&this);
    apply_action.connect_activate(move |_, _| {
        if let Some(this) = weak.upgrade() {
            this.start_save(false);
        }
    });
    window.add_action(&apply_action);

    let weak = Rc::downgrade(&this);
    window.connect_close_request(move |_| {
        let Some(this) = weak.upgrade() else {
            return glib::Propagation::Proceed;
        };
        if this.saving.get() {
            return glib::Propagation::Stop;
        }
        if !this.ctx.store.borrow().is_dirty() {
            return glib::Propagation::Proceed;
        }
        this.confirm_close();
        glib::Propagation::Stop
    });

    sidebar.select_row(sidebar.row_at_index(0).as_ref());
    this.refresh_pages();
    (this.ctx.notify)();
    // 窗口上的回调全是弱引用：窗体自己必须活到窗口销毁，否则 build 返回后
    // Rc 被丢弃，翻页 / 保存 / 关闭确认全部静默失效
    let keep = this.clone();
    window.connect_destroy(move |_| drop(keep.clone()));
    this
}

/// 横幅优先级：配置语法错误（常驻，修好文件重新载入才消）> 一次性通知 > 隐藏。
fn update_banner(
    store: &Rc<RefCell<DraftStore>>,
    banner: &adw::Banner,
    notice: &Rc<RefCell<Option<String>>>,
) {
    let parse_error = store.borrow().snapshot().parse_error.clone();
    if let Some(error) = parse_error {
        banner.set_title(&format!("配置文件有语法错误：{error}；修正前保存会被拒绝"));
        banner.set_revealed(true);
    } else if let Some(message) = notice.borrow().as_ref() {
        banner.set_title(message);
        banner.set_revealed(true);
    } else {
        banner.set_revealed(false);
    }
}

impl SettingsWindow {
    pub fn present(&self) {
        self.window.present();
    }

    fn on_row_selected(&self, row: Option<&gtk::ListBoxRow>) {
        let Some(row) = row else { return };
        let index = row.index();
        if index < 0 {
            return;
        }
        self.stack.set_visible_child_name(&index.to_string());
        self.content_page
            .set_title(self.pages[index as usize].title());
        // 窄窗口收起侧栏时，点分类进到内容页
        self.split.set_show_content(true);
    }

    /// 从当前快照回填全部页面并清掉错误标注。
    fn refresh_pages(&self) {
        let config = self.ctx.store.borrow().config().clone();
        for page in &self.pages {
            page.refresh(&config);
            page.clear_errors();
        }
    }

    fn select_page(&self, index: usize) {
        self.sidebar
            .select_row(self.sidebar.row_at_index(index as i32).as_ref());
    }

    fn show_toast(&self, message: &str) {
        let toast = adw::Toast::new(message);
        toast.set_timeout(4);
        self.toast.add_toast(toast);
    }

    fn show_notice(&self, message: String) {
        *self.notice.borrow_mut() = Some(message);
        update_banner(&self.ctx.store, &self.banner, &self.notice);
    }

    /// 「撤销修改」：清空草稿回到快照。
    fn revert_all(&self) {
        self.ctx.store.borrow_mut().revert();
        *self.notice.borrow_mut() = None;
        self.refresh_pages();
        for page in &self.pages {
            page.on_saved();
        }
        (self.ctx.notify)();
        self.show_toast("已撤销未保存的修改");
    }

    /// 点「应用」/ Ctrl+S / 关闭确认里的「保存」。耗时部分在后台线程，结果回主线程。
    fn start_save(self: &Rc<Self>, close_after: bool) {
        if self.saving.get() {
            return;
        }
        // 没进草稿的非法输入（修饰键输入框标红的项）先修再存
        if let Some(index) = self.pages.iter().position(|page| page.has_invalid_input()) {
            self.show_notice("有输入框的内容不合法（标红的项），请先修正再保存".to_owned());
            self.select_page(index);
            return;
        }
        self.saving.set(true);
        self.stack.set_sensitive(false);
        (self.ctx.notify)();
        let (path, snapshot, patch, appearance) = {
            let store = self.ctx.store.borrow();
            (
                store.path().to_owned(),
                store.snapshot().clone(),
                store.patch().clone(),
                // 外观草稿有改动才走 Fcitx5 那一段
                store
                    .appearance()
                    .is_dirty()
                    .then(|| store.appearance().clone()),
            )
        };
        let admin = self.ctx.admin.clone();
        let this = self.clone();
        glib::MainContext::default().spawn_local(async move {
            let outcome = gio::spawn_blocking(move || {
                save(&path, &snapshot, &patch, &admin, appearance.as_ref())
            })
            .await;
            this.finish_save(outcome, close_after);
        });
    }

    fn finish_save(self: &Rc<Self>, outcome: std::thread::Result<SaveOutcome>, close_after: bool) {
        self.saving.set(false);
        self.stack.set_sensitive(true);
        let outcome = match outcome {
            Ok(outcome) => outcome,
            Err(_) => {
                self.show_notice("保存失败：后台任务中断".to_owned());
                (self.ctx.notify)();
                return;
            }
        };
        match outcome {
            SaveOutcome::DraftBroken(error) => {
                self.show_notice(format!("草稿无法拼成完整配置：{error}"));
            }
            SaveOutcome::Invalid(errors) => self.show_field_errors(&errors),
            SaveOutcome::Conflicts(conflicts) => self.show_conflicts(&conflicts),
            SaveOutcome::WriteFailed(message) => self.show_notice(message),
            SaveOutcome::Saved { server, appearance } => {
                // 落盘成功，推进基准（重新读快照、清空草稿）；外观没应用成功的部分留在草稿里等重试
                let appearance_applied = !matches!(&appearance, Some(ComponentStatus::Failed(_)));
                if let Err(error) = self.ctx.store.borrow_mut().mark_saved(appearance_applied) {
                    self.show_notice(format!("配置已保存，但重新读取配置失败：{error}"));
                    (self.ctx.notify)();
                    return;
                }
                self.refresh_pages();
                for page in &self.pages {
                    page.on_saved();
                }
                *self.notice.borrow_mut() = None;
                // 部分失败如实展示：配置已落盘但服务 / Fcitx5 没应用时不显示成全部成功
                let mut problems = Vec::new();
                if let ComponentStatus::Failed(message) = &server {
                    problems.push(format!("服务应用失败：{message}"));
                }
                if let Some(ComponentStatus::Failed(message)) = &appearance {
                    problems.push(format!("外观未应用：{message}"));
                }
                if problems.is_empty() {
                    match &server {
                        ComponentStatus::Applied => self.show_toast("设置已保存并生效"),
                        ComponentStatus::PendingReload => {
                            self.show_toast("已保存，当前输入结束后生效")
                        }
                        ComponentStatus::Loading => self.show_toast("已保存，模型在后台加载"),
                        ComponentStatus::Saved => {
                            self.show_toast("已保存，服务未运行，下次启动生效")
                        }
                        ComponentStatus::Failed(_) => unreachable!(),
                    }
                } else {
                    self.show_notice(format!("配置已保存，{}", problems.join("；")));
                }
                let any_failed =
                    matches!(server, ComponentStatus::Failed(_)) || !appearance_applied;
                if close_after && !any_failed {
                    // 草稿已清空，close-request 会放行
                    self.window.close();
                }
            }
        }
        (self.ctx.notify)();
    }

    /// 校验未过：标注字段、跳到第一个错误所在页面、横幅汇总。
    fn show_field_errors(&self, errors: &[FieldError]) {
        for page in &self.pages {
            page.clear_errors();
        }
        let mut first_page = None;
        for error in errors {
            for (index, page) in self.pages.iter().enumerate() {
                if page.mark_error(&error.field, &error.message) {
                    if first_page.is_none() {
                        first_page = Some(index);
                    }
                    break;
                }
            }
        }
        if let Some(index) = first_page {
            self.select_page(index);
        }
        let summary = if errors.len() == 1 {
            format!("校验未通过：{}", errors[0].message)
        } else {
            format!(
                "校验未通过：{}（共 {} 处）",
                errors[0].message,
                errors.len()
            )
        };
        self.show_notice(summary);
    }

    /// 保存冲突：列出字段，提供「放弃草稿重新载入」与「保留草稿」。
    fn show_conflicts(self: &Rc<Self>, conflicts: &[FieldConflict]) {
        let mut body = "以下字段在编辑期间被其他程序改过，本次未保存任何修改：".to_owned();
        for conflict in conflicts.iter().take(10) {
            body.push_str(&format!(
                "\n\n{}：当前是 {}，草稿想改成 {}",
                conflict.field,
                conflict.current.as_deref().unwrap_or("（不存在）"),
                conflict.draft
            ));
        }
        let dialog = adw::AlertDialog::builder()
            .heading("配置与外部修改冲突")
            .body(body)
            .build();
        dialog.add_responses(&[("keep", "保留草稿"), ("reload", "放弃草稿并重新载入")]);
        dialog.set_response_appearance("reload", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("keep"));
        dialog.set_close_response("keep");
        let weak = Rc::downgrade(self);
        dialog.connect_response(None, move |_, response| {
            let Some(this) = weak.upgrade() else { return };
            if response != "reload" {
                return;
            }
            match this.ctx.store.borrow_mut().reload() {
                Ok(()) => {
                    *this.notice.borrow_mut() = None;
                    this.refresh_pages();
                    (this.ctx.notify)();
                    this.show_toast("已载入最新配置，草稿已放弃");
                }
                Err(error) => this.show_notice(format!("重新载入失败：{error}")),
            }
        });
        dialog.present(Some(&self.window));
    }

    /// 关闭有未保存修改的窗口：保存并退出 / 放弃修改 / 返回。
    fn confirm_close(self: &Rc<Self>) {
        let dialog = adw::AlertDialog::builder()
            .heading("有未保存的修改")
            .body("关闭窗口前要保存这些修改吗？")
            .build();
        dialog.add_responses(&[
            ("cancel", "返回"),
            ("discard", "放弃修改"),
            ("save", "保存"),
        ]);
        dialog.set_response_appearance("discard", adw::ResponseAppearance::Destructive);
        dialog.set_response_appearance("save", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");
        let weak = Rc::downgrade(self);
        dialog.connect_response(None, move |_, response| {
            let Some(this) = weak.upgrade() else { return };
            match response {
                "save" => this.start_save(true),
                // destroy 不走 close-request，直接放弃草稿关窗
                "discard" => this.window.destroy(),
                _ => {}
            }
        });
        dialog.present(Some(&self.window));
    }
}
