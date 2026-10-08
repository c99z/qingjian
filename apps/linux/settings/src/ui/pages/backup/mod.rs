//! 本机备份和恢复入口，服务协调写入并保留恢复前备份。
mod operations;
use crate::ui::pages::{PageContext, SettingsPage};
use adw::prelude::*;
use qingjian_platform::Config;
use std::rc::Rc;

pub struct BackupPage {
    page: adw::PreferencesPage,
}
impl BackupPage {
    pub fn new(ctx: &PageContext) -> Rc<Self> {
        let page = adw::PreferencesPage::new();
        let group = adw::PreferencesGroup::builder().title("备份与恢复").description("包含配置、学习、统计、短语、用户词库和青简定制主题。API Key、账号凭据、输入日志和模型不进入备份。需要新版本机青简服务正在运行。").build();
        let status = adw::ActionRow::builder()
            .title("操作状态")
            .subtitle("尚未执行")
            .build();
        let export = adw::ActionRow::builder()
            .title("导出备份")
            .subtitle("先将服务中的最新学习数据落盘，再生成 .qjb 文件")
            .build();
        let button = gtk::Button::with_label("导出…");
        button.set_valign(gtk::Align::Center);
        let c = ctx.clone();
        let r = status.clone();
        button.connect_clicked(move |b| operations::export(&c, b, &r));
        export.add_suffix(&button);
        group.add(&export);
        let restore = adw::ActionRow::builder()
            .title("恢复备份")
            .subtitle("立即替换用户数据；恢复前自动备份，云联想保持关闭，本机密钥保留")
            .build();
        let button = gtk::Button::with_label("恢复…");
        button.set_valign(gtk::Align::Center);
        let c = ctx.clone();
        let r = status.clone();
        button.connect_clicked(move |b| operations::restore(&c, b, &r));
        restore.add_suffix(&button);
        group.add(&restore);
        group.add(&status);
        page.add(&group);
        Rc::new(Self { page })
    }
}
impl SettingsPage for BackupPage {
    fn title(&self) -> &'static str {
        "备份与恢复"
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
