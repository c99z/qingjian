//! 词库文件选择、同名确认与后台导入移除。
use super::DictionariesPage;
use crate::adapters::dictionaries;
use adw::prelude::*;
use qingjian_platform::dirs::linux;
use std::rc::Rc;
fn parent(button: &gtk::Button) -> Option<gtk::Window> {
    button.root().and_then(|r| r.downcast().ok())
}

pub(super) fn choose_import(page: &Rc<DictionariesPage>, button: &gtk::Button) {
    let page = page.clone();
    let parent = parent(button);
    gtk::glib::MainContext::default().spawn_local(async move {
        let file = match gtk::FileDialog::builder().title("选择用户词库").build().open_future(parent.as_ref()).await { Ok(file) => file, Err(_) => return };
        let Some(source) = file.path() else { page.status.set_subtitle("请选择本机文件"); return; };
        let stem = source.file_name().unwrap_or_default().to_string_lossy().to_string();
        let stem = [".dict.yaml", ".yaml", ".yml", ".tsv", ".txt", ".qj"].iter().find_map(|suffix| stem.strip_suffix(suffix)).unwrap_or(&stem).to_owned();
        let dialog = adw::AlertDialog::builder().heading("导入用户词库")
            .body("请输入词库名称。同名文件将被替换；可以修改名称保留两份。文件先验证，失败时保留原词库。此操作立即执行。").build();
        let name = gtk::Entry::new(); name.set_text(&stem); dialog.set_extra_child(Some(&name));
        dialog.add_responses(&[("cancel", "返回"), ("import", "导入／替换")]); dialog.set_close_response("cancel");
        dialog.connect_response(None, move |_, response| {
            if response != "import" { return; }
            let name = name.text().to_string(); let page = page.clone(); let source = source.clone();
            run(&page.clone(), move || dictionaries::import(&source, &linux::user_dir(), &name, true));
        });
        dialog.present(parent.as_ref());
    });
}
pub(super) fn confirm_remove(page: &Rc<DictionariesPage>, name: &str, button: &gtk::Button) {
    let dialog = adw::AlertDialog::builder()
        .heading(format!("移除 {name}？"))
        .body("将立即删除用户词库文件，无法撤销。随包词库和学习记录保留。")
        .build();
    dialog.add_responses(&[("cancel", "返回"), ("remove", "移除")]);
    dialog.set_response_appearance("remove", adw::ResponseAppearance::Destructive);
    dialog.set_close_response("cancel");
    let page = page.clone();
    let name = name.to_owned();
    dialog.connect_response(None, move |_, response| {
        if response == "remove" {
            let name = name.clone();
            run(&page, move || {
                dictionaries::remove(&linux::user_dir(), &name)?;
                Ok(format!("已移除 {name}"))
            });
        }
    });
    dialog.present(parent(button).as_ref());
}
fn run(
    page: &Rc<DictionariesPage>,
    action: impl FnOnce() -> Result<String, String> + Send + 'static,
) {
    if page.ctx.busy.replace(true) {
        return;
    }
    (page.ctx.notify)();
    page.status.set_subtitle("正在处理…");
    let page = page.clone();
    let admin = page.ctx.admin.clone();
    gtk::glib::MainContext::default().spawn_local(async move {
        let result = gtk::gio::spawn_blocking(move || {
            let message = action()?;
            let report = match admin.reload() {
                Ok(qingjian_platform::settings::ComponentStatus::Applied) => "已生效".to_owned(),
                Ok(qingjian_platform::settings::ComponentStatus::PendingReload) => {
                    "当前输入结束后生效".to_owned()
                }
                Ok(other) => format!("服务状态：{other:?}"),
                Err(crate::adapters::admin::AdminError::NotRunning) => {
                    "下次启动服务生效".to_owned()
                }
                Err(error) => error.user_message(),
            };
            Ok(format!("{message}；{report}"))
        })
        .await;
        page.status.set_subtitle(&match result {
            Ok(Ok(s)) | Ok(Err(s)) => s,
            Err(_) => "词库操作中断".into(),
        });
        page.ctx.busy.set(false);
        (page.ctx.notify)();
        page.reload_list();
    });
}
