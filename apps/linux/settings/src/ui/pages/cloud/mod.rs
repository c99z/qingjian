//! 云服务设置：AI 地址、模型、本机密钥与独立连接测试。
use crate::ui::pages::{
    PageContext, SettingsPage, SyncGuard, clear_row_error, mark_row_error, switch_row,
};
use adw::prelude::*;
use qingjian_platform::Config;
use std::rc::Rc;
mod test;

pub struct CloudPage {
    page: adw::PreferencesPage,

    ctx: PageContext,

    syncing: SyncGuard,

    enabled: adw::SwitchRow,

    url: adw::EntryRow,

    model: adw::EntryRow,

    key: adw::PasswordEntryRow,

    sentence: adw::SwitchRow,

    slots: adw::SpinRow,

    timeout: adw::SpinRow,

    debounce: adw::SpinRow,

    fields: Vec<(&'static str, adw::PreferencesRow)>,
}
impl CloudPage {
    pub fn new(ctx: &PageContext) -> Rc<Self> {
        let page = adw::PreferencesPage::new();
        let syncing = SyncGuard::default();
        let group = adw::PreferencesGroup::builder()
            .title("AI 云服务")
            .description("支持 OpenAI 兼容接口；默认关闭。此处修改点击底部「应用」后保存。")
            .build();
        let c = ctx.clone();
        let enabled = switch_row(
            "启用云联想",
            "网络故障时继续使用本地候选",
            move |on| c.set("predict", "enabled", on),
            &syncing,
        );
        group.add(&enabled);
        let url = entry("API URL", "base_url", ctx, &syncing);
        url.set_tooltip_text(Some(
            "接口基础地址，例如 https://api.example.com/v1；不包含 /chat/completions",
        ));
        group.add(&url);
        let model = entry("模型名称", "model", ctx, &syncing);
        group.add(&model);
        let key = adw::PasswordEntryRow::builder()
            .title("API Key（留空保留，修改后点应用）")
            .build();
        let c = ctx.clone();
        let guard = syncing.clone();
        key.connect_changed(move |row| {
            if !guard.get() {
                if row.text().is_empty() {
                    c.store.borrow_mut().cancel_credential();
                    (c.notify)();
                    return;
                }
                c.store.borrow_mut().set_credential(row.text().to_string());
                (c.notify)();
            }
        });
        group.add(&key);
        let clear = adw::ActionRow::builder()
            .title("移除本机 API Key")
            .subtitle("点击后进入草稿，应用后移除；环境变量中的密钥仍可能可用")
            .build();
        let button = gtk::Button::with_label("移除密钥");
        button.set_valign(gtk::Align::Center);
        let c = ctx.clone();
        let row = key.clone();
        button.connect_clicked(move |_| {
            row.set_text("");
            c.store.borrow_mut().set_credential(String::new());
            (c.notify)();
        });
        clear.add_suffix(&button);
        group.add(&clear);
        page.add(&group);
        let options = adw::PreferencesGroup::builder()
            .title("联想与隐私")
            .description("Linux 当前只发送拼音、切分和本地候选，不发送光标附近文字或输入历史。")
            .build();
        let c = ctx.clone();
        let sentence = switch_row(
            "整句补全",
            "显示在候选面板的提示区，按 Tab 接受",
            move |on| c.set("predict", "sentence", on),
            &syncing,
        );
        options.add(&sentence);
        let slots = spin("云候选格数", "slots", 0.0, 9.0, ctx, &syncing);
        options.add(&slots);
        let timeout = spin(
            "请求超时（毫秒）",
            "timeout_ms",
            100.0,
            30000.0,
            ctx,
            &syncing,
        );
        options.add(&timeout);
        let debounce = spin(
            "停键防抖（毫秒）",
            "debounce_ms",
            0.0,
            5000.0,
            ctx,
            &syncing,
        );
        options.add(&debounce);
        page.add(&options);
        let probe = adw::PreferencesGroup::builder()
            .title("连接测试")
            .description("使用当前草稿发送固定测试内容；不会保存设置或开启输入联想。")
            .build();
        let status = adw::ActionRow::builder()
            .title("测试 AI 连接")
            .subtitle("尚未测试")
            .build();
        let button = gtk::Button::with_label("测试连接");
        button.set_valign(gtk::Align::Center);
        let c = ctx.clone();
        let status_clone = status.clone();
        button.connect_clicked(move |button| test::start(&c, button, &status_clone));
        status.add_suffix(&button);
        probe.add(&status);
        page.add(&probe);
        let fields = vec![
            ("predict.base_url", url.clone().upcast()),
            ("predict.model", model.clone().upcast()),
            ("predict.slots", slots.clone().upcast()),
            ("predict.timeout_ms", timeout.clone().upcast()),
            ("predict.debounce_ms", debounce.clone().upcast()),
        ];
        Rc::new(Self {
            page,
            ctx: ctx.clone(),
            syncing,
            enabled,
            url,
            model,
            key,
            sentence,
            slots,
            timeout,
            debounce,
            fields,
        })
    }
}
fn entry(
    title: &str,
    field: &'static str,
    ctx: &PageContext,
    syncing: &SyncGuard,
) -> adw::EntryRow {
    let row = adw::EntryRow::builder().title(title).build();
    let c = ctx.clone();
    let guard = syncing.clone();
    row.connect_changed(move |row| {
        if !guard.get() {
            c.set("predict", field, row.text().as_str());
        }
    });
    row
}
fn spin(
    title: &str,
    field: &'static str,
    min: f64,
    max: f64,
    ctx: &PageContext,
    syncing: &SyncGuard,
) -> adw::SpinRow {
    let row = adw::SpinRow::with_range(min, max, 1.0);
    row.set_title(title);
    let c = ctx.clone();
    let guard = syncing.clone();
    row.connect_value_notify(move |row| {
        if !guard.get() {
            c.set("predict", field, row.value() as i64);
        }
    });
    row
}
impl SettingsPage for CloudPage {
    fn title(&self) -> &'static str {
        "云服务"
    }
    fn widget(&self) -> &adw::PreferencesPage {
        &self.page
    }
    fn refresh(&self, config: &Config) {
        self.syncing.set(true);
        self.enabled.set_active(config.predict.enabled);
        self.url.set_text(&config.predict.base_url);
        self.model.set_text(&config.predict.model);
        self.key.set_text(
            self.ctx
                .store
                .borrow()
                .credential()
                .map(|edit| edit.key.as_str())
                .unwrap_or(""),
        );
        self.sentence.set_active(config.predict.sentence);
        self.slots.set_value(config.predict.slots as f64);
        self.timeout.set_value(config.predict.timeout_ms as f64);
        self.debounce.set_value(config.predict.debounce_ms as f64);
        let title = match qingjian_platform::credentials::resolve(
            &config.predict,
            self.ctx.store.borrow().path(),
        ) {
            Ok(config) if config.resolve_api_key().is_some() => "API Key（已配置；留空保留）",
            Ok(_) => "API Key（未配置；填写后点应用）",
            Err(_) => "API Key（本机凭据读取失败）",
        };
        self.key.set_title(title);
        self.syncing.set(false);
    }
    fn clear_errors(&self) {
        for (_, row) in &self.fields {
            clear_row_error(row);
        }
    }
    fn mark_error(&self, field: &str, message: &str) -> bool {
        for (name, row) in &self.fields {
            if *name == field {
                mark_row_error(row, message);
                return true;
            }
        }
        false
    }
}
