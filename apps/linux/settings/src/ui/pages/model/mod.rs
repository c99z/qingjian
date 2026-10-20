//! 本地模型页：`[model] enabled` 开关（走草稿），以及从管理 socket 读到的实际加载状态。
//! 状态查询在后台线程做，结果回主线程更新；服务没跑就如实显示「服务未运行」。

mod status_rows;

use adw::prelude::{
    ButtonExt, PreferencesGroupExt, PreferencesPageExt, PreferencesRowExt, WidgetExt,
};
use qingjian_platform::Config;
use status_rows::StatusRows;

use crate::ui::pages::{
    PageContext, SettingsPage, SyncGuard, clear_row_error, mark_row_error, switch_row,
};

pub struct ModelPage {
    page: adw::PreferencesPage,

    ctx: PageContext,

    syncing: SyncGuard,

    enabled: adw::SwitchRow,

    context_ranking: adw::SwitchRow,

    context_model: adw::ActionRow,

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
        let ctx_context = ctx.clone();
        let context_ranking = switch_row(
            "结合最近输入优化候选（实验性）",
            "使用本机知微模型；默认关闭，失焦或删除后重新积累前文",
            move |on| ctx_context.set("model", "context_ranking", on),
            &syncing,
        );
        let context_sensitive = context_ranking.clone();
        enabled.connect_active_notify(move |row| context_sensitive.set_sensitive(row.is_active()));
        toggle.add(&context_ranking);
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
        model.set_title("基础模型");
        status.add(&model);
        let context_model = adw::ActionRow::new();
        context_model.set_title("语境模型");
        status.add(&context_model);
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
            context_ranking,
            context_model,
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
            context_model: self.context_model.clone(),
            failure: self.failure.clone(),
            admin: self.ctx.admin.clone(),
        }
    }

    fn query_status(&self) {
        self.handle().query_status();
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
        self.context_ranking
            .set_active(config.model.context_ranking);
        self.context_ranking.set_sensitive(config.model.enabled);
        self.syncing.set(false);
    }

    fn clear_errors(&self) {
        clear_row_error(&self.enabled);
        clear_row_error(&self.context_ranking);
    }

    fn mark_error(&self, field: &str, message: &str) -> bool {
        let row = match field {
            "model.enabled" => &self.enabled,
            "model.context_ranking" => &self.context_ranking,
            _ => return false,
        };
        mark_row_error(row, message);
        true
    }
}
