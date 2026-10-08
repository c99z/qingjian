//! 账号操作与同步确认；后台连接不阻塞 GTK 主线程。
use super::AccountPage;
use crate::adapters::{
    account::{self, AccountClient},
    maintenance,
};
use adw::prelude::*;
use qingjian_platform::backup::Archive;
use std::rc::Rc;

pub(super) fn authenticate(page: &Rc<AccountPage>, register: bool) {
    let url = page.url.text().to_string();
    let username = page.username.text().to_string();
    let password = page.password.text().to_string();
    if !page.start(if register {
        "正在注册…"
    } else {
        "正在登录…"
    }) {
        return;
    }
    let page = page.clone();
    gtk::glib::MainContext::default().spawn_local(async move {
        let result = gtk::gio::spawn_blocking(move || {
            let client = AccountClient::new(&url)?;
            let session = client.login(username, password, register)?;
            if let Err(error) = account::save_session(&account::session_path(), &session) {
                let _ = client.logout(&session);
                return Err(error);
            }
            Ok::<_, String>(session)
        })
        .await;
        match result {
            Ok(Ok(session)) => {
                page.url.set_text(&session.server_url);
                page.username.set_text(&session.username);
                page.password.set_text("");
                *page.session.borrow_mut() = Some(session);
                page.finish("登录成功");
                page.show_session();
            }
            Ok(Err(message)) => page.finish(&message),
            Err(_) => page.finish("账号操作中断"),
        }
    });
}
pub(super) fn logout(page: &Rc<AccountPage>) {
    let Some(session) = page.session.borrow().clone() else {
        return;
    };
    if !page.start("正在退出登录…") {
        return;
    }
    let page = page.clone();
    gtk::glib::MainContext::default().spawn_local(async move {
        let result = gtk::gio::spawn_blocking(move || {
            let revoked =
                AccountClient::new(&session.server_url).and_then(|client| client.logout(&session));
            match std::fs::remove_file(account::session_path()) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => return Err("无法清除本机登录凭据".to_owned()),
            }
            Ok(revoked.is_ok())
        })
        .await;
        match result {
            Ok(Ok(revoked)) => {
                *page.session.borrow_mut() = None;
                page.finish(if revoked {
                    "已退出登录"
                } else {
                    "已清除本机登录；远程会话未确认注销，将在到期后失效"
                });
            }
            Ok(Err(message)) => page.finish(&message),
            Err(_) => page.finish("退出操作中断"),
        }
    });
}
pub(super) fn confirm_sync(page: &Rc<AccountPage>, downloading: bool) {
    if page.ctx.store.borrow().is_dirty() {
        page.status
            .set_subtitle("请先应用或撤销设置草稿，再同步数据");
        return;
    }
    let dialog = adw::AlertDialog::builder().heading(if downloading { "下载并恢复云端数据？" } else { "上传并替换云端快照？" })
        .body(if downloading { "云端快照将替换本机配置和学习数据。恢复前自动备份，有未上屏输入时拒绝恢复。云联想保持关闭，本机密钥保留。" } else { "将本机已保存的配置和学习数据上传为此账号的最新快照。不会合并其他设备的数据；版本冲突时拒绝覆盖。" }).build();
    dialog.add_responses(&[
        ("cancel", "返回"),
        (
            "sync",
            if downloading {
                "下载并恢复"
            } else {
                "上传"
            },
        ),
    ]);
    dialog.set_close_response("cancel");
    if downloading {
        dialog.set_response_appearance("sync", adw::ResponseAppearance::Destructive);
    }
    let this = page.clone();
    dialog.connect_response(None, move |_, response| {
        if response == "sync" {
            sync(&this, downloading);
        }
    });
    let parent = page
        .page
        .root()
        .and_then(|r| r.downcast::<gtk::Window>().ok());
    dialog.present(parent.as_ref());
}
fn sync(page: &Rc<AccountPage>, downloading: bool) {
    if page.ctx.store.borrow().is_dirty() {
        page.status.set_subtitle("有未保存的草稿，请先处理");
        return;
    }
    let Some(mut session) = page.session.borrow().clone() else {
        return;
    };
    if !page.start(if downloading {
        "正在下载、验证并恢复…"
    } else {
        "正在备份并上传…"
    }) {
        return;
    }
    let admin = page.ctx.admin.clone();
    let page = page.clone();
    gtk::glib::MainContext::default().spawn_local(async move {
        let result = gtk::gio::spawn_blocking(move || {
            let client = AccountClient::new(&session.server_url)?;
            if downloading {
                let snapshot = client.download(&session)?;
                let file = qingjian_platform::dirs::linux::user_dir()
                    .join("backups")
                    .join(format!("from-cloud-{}.qjb", uuid::Uuid::new_v4()));
                snapshot.archive.write(&file)?;
                maintenance::restore(&admin, &snapshot.archive)?;
                session.revision = snapshot.revision;
            } else {
                let file = admin.backup().map_err(|e| e.user_message())?;
                session.revision = client.upload(&session, Archive::read(&file)?)?;
            }
            let persisted = account::save_session(&account::session_path(), &session).is_ok();
            Ok::<_, String>((session, persisted))
        })
        .await;
        match result {
            Ok(Ok((session, persisted))) => {
                *page.session.borrow_mut() = Some(session);
                if downloading {
                    let result = page.ctx.store.borrow_mut().reload();
                    if result.is_err() {
                        page.finish("数据已恢复，但配置回读失败，请重新打开设置窗口");
                        return;
                    }
                    (page.ctx.refresh)();
                }
                page.finish(if !persisted {
                    "同步已完成，但本机会话版本保存失败；下次打开窗口后请重新登录"
                } else if downloading {
                    "云端数据已恢复；原数据备份保存在学习目录 backups 中"
                } else {
                    "上传成功，其他设备可登录此账号下载恢复"
                });
            }
            Ok(Err(message)) => page.finish(&message),
            Err(_) => page.finish("同步任务中断，请检查本机服务与云端状态"),
        }
    });
}
