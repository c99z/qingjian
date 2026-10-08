//! 文件选择、恢复确认和后台数据维护。
use crate::adapters::maintenance;
use crate::ui::pages::PageContext;
use adw::prelude::*;
use qingjian_platform::backup::Archive;

fn parent(button: &gtk::Button) -> Option<gtk::Window> {
    button.root().and_then(|r| r.downcast().ok())
}
pub(super) fn export(ctx: &PageContext, button: &gtk::Button, status: &adw::ActionRow) {
    let c = ctx.clone();
    let b = button.clone();
    let r = status.clone();
    let parent = parent(button);
    gtk::glib::MainContext::default().spawn_local(async move {
        let file = match gtk::FileDialog::builder()
            .title("导出青简备份")
            .initial_name("qingjian-backup.qjb")
            .build()
            .save_future(parent.as_ref())
            .await
        {
            Ok(file) => file,
            Err(_) => return,
        };
        let Some(path) = file.path() else {
            r.set_subtitle("请选择本机文件");
            return;
        };
        b.set_sensitive(false);
        r.set_subtitle("正在生成一致性备份…");
        let admin = c.admin.clone();
        let result = gtk::gio::spawn_blocking(move || maintenance::export(&admin, &path)).await;
        r.set_subtitle(&match result {
            Ok(Ok(())) => "备份已导出".into(),
            Ok(Err(e)) => e,
            Err(_) => "备份任务中断".into(),
        });
        b.set_sensitive(true);
    });
}
pub(super) fn restore(ctx: &PageContext, button: &gtk::Button, status: &adw::ActionRow) {
    if ctx.store.borrow().is_dirty() {
        status.set_subtitle("请先应用或撤销设置草稿，再恢复数据");
        return;
    }
    let c = ctx.clone();
    let b = button.clone();
    let r = status.clone();
    let parent = parent(button);
    gtk::glib::MainContext::default().spawn_local(async move {
        let file = match gtk::FileDialog::builder().title("选择青简备份").build().open_future(parent.as_ref()).await { Ok(file) => file, Err(_) => return };
        let Some(path) = file.path() else { r.set_subtitle("请选择本机文件"); return; };
        let dialog = adw::AlertDialog::builder().heading("恢复此备份？").body("将立即替换配置和用户学习数据。服务会保留恢复前备份；还有未上屏的输入时拒绝恢复。恢复后自动载入恢复的配置。").build();
        dialog.add_responses(&[("cancel", "返回"), ("restore", "恢复")]); dialog.set_close_response("cancel"); dialog.set_response_appearance("restore", adw::ResponseAppearance::Destructive);
        dialog.connect_response(None, move |_, response| {
            if response != "restore" { return; }
            if c.store.borrow().is_dirty() { r.set_subtitle("有未保存的草稿，请先处理"); return; }
            let admin = c.admin.clone(); let path = path.clone(); let b = b.clone(); let r = r.clone(); let c = c.clone(); c.busy.set(true); (c.notify)(); b.set_sensitive(false); r.set_subtitle("正在验证并恢复…");
            gtk::glib::MainContext::default().spawn_local(async move {
                let result = gtk::gio::spawn_blocking(move || maintenance::restore(&admin, &Archive::read(&path)?)).await;
                let success = matches!(result, Ok(Ok(())));
                r.set_subtitle(&match result { Ok(Ok(())) => "已恢复，原数据备份保存在学习目录 backups 中".into(), Ok(Err(e)) => e, Err(_) => "恢复任务中断，请检查服务状态".into() });
                if success { match c.store.borrow_mut().reload() { Ok(()) => {}, Err(_) => r.set_subtitle("数据已恢复，配置回读失败，请重新打开设置窗口") } (c.refresh)(); }
                c.busy.set(false); (c.notify)(); b.set_sensitive(true);
            });
        }); dialog.present(parent.as_ref());
    });
}
