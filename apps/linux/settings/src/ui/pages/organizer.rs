//! 学习页里的夜间整理控件；独立授权、执行时间、专业提示和上次结果。

use adw::prelude::{ActionRowExt, Cast, EditableExt, PreferencesGroupExt, PreferencesRowExt};
use qingjian_platform::{Config, dirs::linux};

use super::{PageContext, SyncGuard, switch_row};

pub(super) struct OrganizerControls {
    pub group: adw::PreferencesGroup,

    enabled: adw::SwitchRow,

    hour: adw::SpinRow,

    minute: adw::SpinRow,

    hint: adw::EntryRow,

    status: adw::ActionRow,
}

impl OrganizerControls {
    pub fn new(ctx: &PageContext, guard: &SyncGuard) -> Self {
        let group = adw::PreferencesGroup::builder().title("夜间整理个人词库")
            .description("复用「云服务」的 API 地址、模型与密钥，与云联想开关独立。仅发送本机筛选出的重复词、读音和次数，不发送整份日志。需开启学习和输入日志；私密输入不参与。")
            .build();
        let c = ctx.clone();
        let enabled = switch_row(
            "夜间自动整理个人词库",
            "默认关闭；开启后会向所配置的 AI 服务发送上述用词证据，并产生 API 费用",
            move |on| c.set("organizer", "enabled", on),
            guard,
        );
        group.add(&enabled);
        let hour = Self::spin("整理时间：小时（0–23）", "hour", 23.0, ctx, guard);
        let minute = Self::spin("整理时间：分钟（0–59）", "minute", 59.0, ctx, guard);
        group.add(&hour);
        group.add(&minute);
        let hint = adw::EntryRow::builder()
            .title("专业领域提示（可选，最多 120 字）")
            .build();
        let c = ctx.clone();
        let g = guard.clone();
        hint.connect_changed(move |row| {
            if !g.get() {
                c.set("organizer", "domain_hint", row.text().as_str());
            }
        });
        group.add(&hint);
        let timing = adw::ActionRow::builder().title("执行条件")
            .subtitle("按本机时间，每天最多尝试一次；电脑和青简服务须运行。错过后 4 小时内补做，正在输入时等待结束。失败保留原词库，次日再试。")
            .build();
        group.add(&timing);
        let status = adw::ActionRow::builder()
            .title("最近整理结果")
            .subtitle("尚未整理")
            .build();
        group.add(&status);
        Self {
            group,
            enabled,
            hour,
            minute,
            hint,
            status,
        }
    }

    fn spin(
        title: &str,
        field: &'static str,
        max: f64,
        ctx: &PageContext,
        guard: &SyncGuard,
    ) -> adw::SpinRow {
        let row = adw::SpinRow::with_range(0.0, max, 1.0);
        row.set_title(title);
        let c = ctx.clone();
        let g = guard.clone();
        row.connect_value_notify(move |row| {
            if !g.get() {
                c.set("organizer", field, row.value() as i64);
            }
        });
        row
    }

    pub fn fields(&self) -> Vec<(&'static str, adw::PreferencesRow)> {
        vec![
            ("organizer.hour", self.hour.clone().upcast()),
            ("organizer.minute", self.minute.clone().upcast()),
            ("organizer.domain_hint", self.hint.clone().upcast()),
        ]
    }

    pub fn refresh(&self, config: &Config) {
        self.enabled.set_active(config.organizer.enabled);
        self.hour.set_value(f64::from(config.organizer.hour));
        self.minute.set_value(f64::from(config.organizer.minute));
        self.hint.set_text(&config.organizer.domain_hint);
        let allowed = config.general.learning && config.general.input_log;
        self.enabled.set_subtitle(if allowed {
            "开启后发送筛选后的词、读音和次数到所配置 AI 服务，并产生 API 费用"
        } else {
            "需开启学习和输入日志；当前不会执行整理"
        });
        if let Ok(bytes) = std::fs::read(linux::user_dir().join("organizer/status.json"))
            && let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes)
        {
            self.status.set_subtitle(&format!(
                "{} · {}",
                value["time"].as_str().unwrap_or(""),
                value["message"].as_str().unwrap_or("")
            ));
        }
    }
}
