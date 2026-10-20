//! 本地基础模型与语境模型的独立运行状态。

use crate::adapters::admin::AdminError;
use adw::prelude::{ActionRowExt, WidgetExt};
use gtk::glib;
use qingjian_platform::protocol::linux_admin::ModelStatus;

/// 状态查询要动的几行；查询在后台线程做，完成后回主线程更新。
#[derive(Clone)]
pub(super) struct StatusRows {
    pub(super) service: adw::ActionRow,

    pub(super) config: adw::ActionRow,

    pub(super) model: adw::ActionRow,

    pub(super) context_model: adw::ActionRow,

    pub(super) failure: adw::ActionRow,

    pub(super) admin: std::sync::Arc<crate::adapters::admin::AdminClient>,
}

impl StatusRows {
    pub(super) fn query_status(&self) {
        self.service.set_subtitle("查询中…");
        self.config.set_subtitle("");
        self.model.set_subtitle("");
        self.context_model.set_subtitle("—");
        self.failure.set_visible(false);
        let rows = self.clone();
        glib::MainContext::default().spawn_local(async move {
            let result = gtk::gio::spawn_blocking(move || rows.admin.status()).await;
            match result {
                Ok(Ok(report)) => {
                    rows.service
                        .set_subtitle(&format!("运行中（版本 {}）", report.version));
                    let mut config = format!("代次 {}", report.config.generation);
                    if report.config.pending {
                        config.push_str("，有修改等当前输入结束后生效");
                    }
                    if let Some(error) = &report.config.last_error {
                        config.push_str(&format!("；上次重载出错：{error}"));
                    }
                    rows.config.set_subtitle(&config);
                    rows.context_model
                        .set_subtitle(&match &report.context_model {
                            None => "服务未提供语境状态".to_owned(),
                            Some(ModelStatus::Disabled) => "未启用".to_owned(),
                            Some(ModelStatus::Loading) => "后台加载中…".to_owned(),
                            Some(ModelStatus::Loaded) => "已加载，仅使用最近连续输入".to_owned(),
                            Some(ModelStatus::Failed(reason)) => {
                                format!("{}；使用基础排序", reason)
                            }
                        });
                    match &report.model {
                        ModelStatus::Disabled => rows.model.set_subtitle("未启用"),
                        ModelStatus::Loading => rows.model.set_subtitle("后台加载中…"),
                        ModelStatus::Loaded => rows.model.set_subtitle("已加载"),
                        ModelStatus::Failed(reason) => {
                            rows.model.set_subtitle("加载失败");
                            rows.failure.set_subtitle(reason);
                            rows.failure.set_visible(true);
                        }
                    }
                }
                Ok(Err(AdminError::NotRunning)) => {
                    rows.service.set_subtitle("服务未运行");
                    rows.config.set_subtitle("—");
                    rows.model.set_subtitle("—");
                }
                Ok(Err(error)) => {
                    rows.service
                        .set_subtitle(&format!("查询失败：{}", error.user_message()));
                    rows.config.set_subtitle("—");
                    rows.model.set_subtitle("—");
                }
                Err(_) => rows.service.set_subtitle("查询失败：后台任务中断"),
            }
        });
    }
}
