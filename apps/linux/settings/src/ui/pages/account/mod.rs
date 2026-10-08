//! 登录、注册和跨设备快照同步入口，账号服务地址与 AI URL 分离。
mod operations;
use crate::adapters::account;
use crate::ui::pages::{PageContext, SettingsPage};
use adw::prelude::*;
use qingjian_platform::{Config, account::AccountSession};
use std::cell::RefCell;
use std::rc::Rc;

pub struct AccountPage {
    page: adw::PreferencesPage,

    ctx: PageContext,

    url: adw::EntryRow,

    username: adw::EntryRow,

    password: adw::PasswordEntryRow,

    status: adw::ActionRow,

    session: RefCell<Option<AccountSession>>,

    upload: gtk::Button,

    download: gtk::Button,

    logout: gtk::Button,
}
impl AccountPage {
    pub fn new(ctx: &PageContext) -> Rc<Self> {
        let page = adw::PreferencesPage::new();
        let group = adw::PreferencesGroup::builder().title("自建账号服务").description("登录用于备份和跨设备同步，本地输入无需登录。请填写你部署的账号服务地址；此地址和 AI API URL 独立。远程地址需要 HTTPS，本机测试可用 HTTP。").build();
        let url = adw::EntryRow::builder()
            .title("账号服务 URL")
            .text("http://127.0.0.1:9080")
            .build();
        group.add(&url);
        let username = adw::EntryRow::builder().title("用户名").build();
        group.add(&username);
        let password = adw::PasswordEntryRow::builder()
            .title("密码（注册时至少 12 字节）")
            .build();
        group.add(&password);
        let auth = adw::ActionRow::builder().title("账号操作").build();
        let login = gtk::Button::with_label("登录");
        login.add_css_class("suggested-action");
        login.set_valign(gtk::Align::Center);
        auth.add_suffix(&login);
        let register = gtk::Button::with_label("注册");
        register.set_valign(gtk::Align::Center);
        auth.add_suffix(&register);
        let logout = gtk::Button::with_label("退出登录");
        logout.set_valign(gtk::Align::Center);
        auth.add_suffix(&logout);
        group.add(&auth);
        let status = adw::ActionRow::builder()
            .title("账号与同步状态")
            .subtitle("未登录")
            .build();
        group.add(&status);
        page.add(&group);
        let sync = adw::PreferencesGroup::builder().title("跨设备同步").description("通过上传快照、下载并恢复，在设备间同步配置、学习、统计、词库和短语。手动触发；不自动合并。云端版本冲突时拒绝覆盖。AI Key、账号凭据和输入日志不会上传。").build();
        let upload_row = adw::ActionRow::builder()
            .title("上传本机数据")
            .subtitle("覆盖此账号的云端快照；先生成本机一致性备份")
            .build();
        let upload = gtk::Button::with_label("上传备份");
        upload.set_valign(gtk::Align::Center);
        upload_row.add_suffix(&upload);
        sync.add(&upload_row);
        let download_row = adw::ActionRow::builder()
            .title("下载云端数据并恢复")
            .subtitle("替换本机数据；恢复前自动备份，云联想保持关闭")
            .build();
        let download = gtk::Button::with_label("下载并恢复");
        download.set_valign(gtk::Align::Center);
        download_row.add_suffix(&download);
        sync.add(&download_row);
        page.add(&sync);
        let session = account::load_session(&account::session_path());
        let this = Rc::new(Self {
            page,
            ctx: ctx.clone(),
            url,
            username,
            password,
            status,
            session: RefCell::new(session.as_ref().ok().and_then(Clone::clone)),
            upload,
            download,
            logout,
        });
        if let Some(session) = this.session.borrow().as_ref() {
            this.url.set_text(&session.server_url);
            this.username.set_text(&session.username);
        }
        if let Err(message) = session {
            this.status.set_subtitle(&message);
        } else {
            this.show_session();
        }
        this.update_buttons();
        for (button, is_register) in [(login, false), (register, true)] {
            let weak = Rc::downgrade(&this);
            button.connect_clicked(move |_| {
                if let Some(this) = weak.upgrade() {
                    operations::authenticate(&this, is_register);
                }
            });
        }
        let weak = Rc::downgrade(&this);
        this.logout.connect_clicked(move |_| {
            if let Some(this) = weak.upgrade() {
                operations::logout(&this);
            }
        });
        for (button, downloading) in [(this.upload.clone(), false), (this.download.clone(), true)] {
            let weak = Rc::downgrade(&this);
            button.connect_clicked(move |_| {
                if let Some(this) = weak.upgrade() {
                    operations::confirm_sync(&this, downloading);
                }
            });
        }
        let weak = Rc::downgrade(&this);
        this.url.connect_changed(move |_| {
            if let Some(this) = weak.upgrade() {
                this.update_buttons();
            }
        });
        this
    }
    fn update_buttons(&self) {
        let logged = self.session.borrow().as_ref().is_some_and(|s| {
            s.server_url.trim_end_matches('/') == self.url.text().trim().trim_end_matches('/')
        });
        self.upload.set_sensitive(logged);
        self.download.set_sensitive(logged);
        self.logout.set_sensitive(self.session.borrow().is_some());
    }
    fn show_session(&self) {
        self.status
            .set_subtitle(&self.session.borrow().as_ref().map_or_else(
                || "未登录".into(),
                |s| format!("已登录：{} · 已确认云端版本 {}", s.username, s.revision),
            ));
    }
    fn start(&self, message: &str) -> bool {
        if self.ctx.busy.get() {
            return false;
        }
        self.ctx.busy.set(true);
        (self.ctx.notify)();
        self.status.set_subtitle(message);
        true
    }
    fn finish(&self, message: &str) {
        self.ctx.busy.set(false);
        (self.ctx.notify)();
        self.status.set_subtitle(message);
        self.update_buttons();
    }
}
impl SettingsPage for AccountPage {
    fn title(&self) -> &'static str {
        "账号与同步"
    }
    fn widget(&self) -> &adw::PreferencesPage {
        &self.page
    }
    fn refresh(&self, _: &Config) {}
    fn clear_errors(&self) {}
    fn mark_error(&self, _: &str, _: &str) -> bool {
        false
    }
}
