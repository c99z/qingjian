//! 本地模型页：`[model] enabled` 开关（走草稿），以及从管理 socket 读到的实际加载状态。
//! 状态查询在后台线程做，结果回主线程更新；服务没跑就如实显示「服务未运行」。

use adw::prelude::*;
use gtk::glib;
use qingjian_platform::Config;
use qingjian_platform::protocol::linux_admin::ModelStatus;

use crate::adapters::admin::AdminError;

use super::{PageContext, SettingsPage, SyncGuard, clear_row_error, mark_row_error, switch_row};

pub struct ModelPage {
    page: adw::PreferencesPage,

    ctx: PageContext,

    syncing: SyncGuard,

    enabled: adw::SwitchRow,

    /// 服务状态（运行中 / 未运行 / 查询失败）。
    service: adw::ActionRow,

    /// 配置代次与挂起状态。
    config: adw::ActionRow,

    /// 模型加载状态。
    model: adw::ActionRow,

    /// 加载失败原因；没有失败时隐藏。
    failure: adw::ActionRow,
}

impl ModelPage {
    pub fn new(ctx: &PageContext) -> Self {
        let page = adw::PreferencesPage::new();
        let syncing = SyncGuard::default();

        let toggle = adw::PreferencesGroup::new();
        toggle.set_title("本地整句模型");
        toggle.set_description(Some(
            "随包的小模型在本机给整句候选重新排序，全程离线、不联网；改动走草稿，点「应用」后保存",
        ));
        let ctx_toggle = ctx.clone();
        let enabled = switch_row(
            "启用本地模型",
            "关掉后只用词库统计排序；模型文件缺失时开关无效",
            move |on| ctx_toggle.set("model", "enabled", on),
            &syncing,
        );
        toggle.add(&enabled);
        page.add(&toggle);

        let status = adw::PreferencesGroup::new();
        status.set_title("运行状态");
        status.set_description(Some("来自青简服务的管理接口，反映实际加载结果"));
        let refresh = gtk::Button::from_icon_name("view-refresh-symbolic");
        refresh.set_tooltip_text(Some("刷新状态"));
        refresh.set_valign(gtk::Align::Center);
        refresh.add_css_class("flat");
        status.set_header_suffix(Some(&refresh));

        let service = adw::ActionRow::new();
        service.set_title("服务状态");
        status.add(&service);
        let config = adw::ActionRow::new();
        config.set_title("配置");
        status.add(&config);
        let model = adw::ActionRow::new();
        model.set_title("模型");
        status.add(&model);
        let failure = adw::ActionRow::new();
        failure.set_title("失败原因");
        failure.set_visible(false);
        status.add(&failure);
        page.add(&status);

        let this = Self {
            page,
            ctx: ctx.clone(),
            syncing,
            enabled,
            service,
            config,
            model,
            failure,
        };
        // 进页面与点刷新都查一次
        let handle = this.handle();
        refresh.connect_clicked(move |_| handle.query_status());
        let handle = this.handle();
        this.page.connect_map(move |_| handle.query_status());
        this.query_status();
        this
    }

    /// 状态行控件的共享句柄（查询回调用），避免把 Self 包进 Rc。
    fn handle(&self) -> StatusRows {
        StatusRows {
            service: self.service.clone(),
            config: self.config.clone(),
            model: self.model.clone(),
            failure: self.failure.clone(),
            admin: self.ctx.admin.clone(),
        }
    }

    fn query_status(&self) {
        self.handle().query_status();
    }
}

/// 状态查询要动的几行；查询在后台线程做，完成后回主线程更新。
#[derive(Clone)]
struct StatusRows {
    service: adw::ActionRow,

    config: adw::ActionRow,

    model: adw::ActionRow,

    failure: adw::ActionRow,

    admin: std::sync::Arc<crate::adapters::admin::AdminClient>,
}

impl StatusRows {
    fn query_status(&self) {
        self.service.set_subtitle("查询中…");
        self.config.set_subtitle("");
        self.model.set_subtitle("");
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

impl SettingsPage for ModelPage {
    fn title(&self) -> &'static str {
        "本地模型"
    }

    fn widget(&self) -> &adw::PreferencesPage {
        &self.page
    }

    fn refresh(&self, config: &Config) {
        self.syncing.set(true);
        self.enabled.set_active(config.model.enabled);
        self.syncing.set(false);
    }

    fn clear_errors(&self) {
        clear_row_error(&self.enabled);
    }

    fn mark_error(&self, field: &str, message: &str) -> bool {
        if field != "model.enabled" {
            return false;
        }
        mark_row_error(&self.enabled, message);
        true
    }
}
