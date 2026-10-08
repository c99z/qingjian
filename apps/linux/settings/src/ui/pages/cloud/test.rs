//! 使用云配置草稿和未保存密钥做固定内容测试，不落盘。
use crate::ui::pages::PageContext;
use adw::prelude::*;
pub fn start(ctx: &PageContext, button: &gtk::Button, status: &adw::ActionRow) {
    let (config, path, credential) = {
        let store = ctx.store.borrow();
        let config = match store.draft() {
            Ok(c) => c.predict,
            Err(_) => {
                status.set_subtitle("草稿配置不合法");
                return;
            }
        };
        (
            config,
            store.path().to_owned(),
            store.credential().map(|edit| edit.key.clone()),
        )
    };
    button.set_sensitive(false);
    status.set_subtitle("正在测试…");
    let button = button.clone();
    let status = status.clone();
    gtk::glib::MainContext::default().spawn_local(async move {
        let result = gtk::gio::spawn_blocking(move || {
            let mut config = qingjian_platform::credentials::resolve(&config, &path)?;
            if let Some(key) = credential {
                config.api_key = Some(key);
            }
            let test = qingjian_predict::ConnectionTest::start(&config)
                .map_err(|_| "无法发起测试：请检查密钥和连接参数".to_owned())?;
            let deadline = std::time::Instant::now()
                + std::time::Duration::from_millis(config.timeout_ms + 2000);
            loop {
                if let Some(result) = test.poll() {
                    return result
                        .map(|r| format!("连接成功：{}，{} 毫秒", r.model, r.elapsed.as_millis()))
                        .map_err(|_| "连接失败：请检查 URL、模型、API Key 和网络".to_owned());
                }
                if std::time::Instant::now() > deadline {
                    return Err("连接测试超时".to_owned());
                }
                std::thread::sleep(std::time::Duration::from_millis(30));
            }
        })
        .await;
        status.set_subtitle(&match result {
            Ok(Ok(message)) | Ok(Err(message)) => message,
            Err(_) => "测试任务中断".into(),
        });
        button.set_sensitive(true);
    });
}
