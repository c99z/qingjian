//! Fcitx5 的 D-Bus 客户端：`org.fcitx.Fcitx5` `/controller` `org.fcitx.Fcitx.Controller1`
//! （注意接口名不带 5）。全部同步阻塞调用（带超时），调用方放到后台线程；GTK 无关，可单测的部分
//! （解析、计划、主题复制）都在兄弟模块里，这里只管收发。

use gio::prelude::*;
use gio::{self, glib};

use super::error::Fcitx5Error;
use super::font::FontDesc;
use super::probe::{self, PanelProbe};
use super::snapshot::ClassicUiState;
use super::theme::{CommentStyle, InstalledTheme};
use super::value::{ConfigMap, enum_options};

const BUS_NAME: &str = "org.fcitx.Fcitx5";
const OBJECT_PATH: &str = "/controller";
const INTERFACE: &str = "org.fcitx.Fcitx.Controller1";

/// classicui 插件配置与主题子配置的 URI 前缀。
pub const CLASSIC_UI_URI: &str = "fcitx://config/addon/classicui";

/// 单次 D-Bus 调用的超时：本地总线上的读写在毫秒级，5 秒没回就当对端卡住。
const TIMEOUT_MS: i32 = 5000;

pub struct Fcitx5Client {
    conn: gio::DBusConnection,
}

impl Fcitx5Client {
    /// 连会话总线并确认 Fcitx5 在运行（没有名字属主报 [`Fcitx5Error::NotRunning`]）。
    pub fn connect() -> Result<Self, Fcitx5Error> {
        let conn = gio::bus_get_sync(gio::BusType::Session, None::<&gio::Cancellable>)?;
        let reply = conn.call_sync(
            Some("org.freedesktop.DBus"),
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
            "NameHasOwner",
            Some(&(BUS_NAME,).to_variant()),
            None,
            gio::DBusCallFlags::NONE,
            TIMEOUT_MS,
            None::<&gio::Cancellable>,
        )?;
        if !reply.child_value(0).get::<bool>().unwrap_or(false) {
            return Err(Fcitx5Error::NotRunning);
        }
        Ok(Self { conn })
    }

    /// 面板检测：经典面板是否启用、有没有别的界面插件。
    pub fn probe_panels(&self) -> Result<PanelProbe, Fcitx5Error> {
        let reply = self.call("GetAddonsV2", None)?;
        Ok(probe::probe_addons(&probe::parse_addons(
            &reply.child_value(0),
        )))
    }

    /// 读经典面板当前配置（含主题列表与当前主题的释义样式）。
    pub fn classic_ui(&self) -> Result<ClassicUiState, Fcitx5Error> {
        let (map, description) = self.get_config(CLASSIC_UI_URI)?;
        let themes = enum_options(&description, "Theme")
            .into_iter()
            .map(|(name, label)| InstalledTheme { name, label })
            .collect();
        let theme = map.text("Theme").unwrap_or("default").to_owned();
        let comment = self.theme_style(&theme)?;
        Ok(ClassicUiState {
            font: FontDesc::parse(map.text("Font").unwrap_or("Sans 10")),
            theme,
            vertical: map.boolean("Vertical Candidate List").unwrap_or(false),
            follow_system_theme: map.boolean("UseDarkTheme").unwrap_or(false),
            themes,
            comment,
        })
    }

    /// 读主题的实时值，包含 Fcitx5 实际采用的缺省值。
    pub fn theme_style(&self, name: &str) -> Result<CommentStyle, Fcitx5Error> {
        if !super::theme::valid_theme_name(name) {
            return Err(Fcitx5Error::ThemeNotFound(name.to_owned()));
        }
        let map = self
            .get_config(&format!("{CLASSIC_UI_URI}/theme/{name}"))?
            .0;
        let panel = map
            .child("InputPanel")
            .ok_or_else(|| Fcitx5Error::BadReply("缺少主题 InputPanel".to_owned()))?;
        let text = |key| panel.text(key).filter(|v| !v.is_empty()).map(str::to_owned);
        Ok(CommentStyle {
            color: panel
                .child("CandidateCommentColor")
                .and_then(|map| map.text("Value").map(str::to_owned)),
            highlight_color: panel
                .child("HighlightCandidateCommentColor")
                .and_then(|map| map.text("Value").map(str::to_owned)),
            size_factor: panel
                .text("CommentTextSizeFactor")
                .and_then(|v| v.parse().ok()),
            normal_color: text("NormalColor"),
            highlight_color_text: text("HighlightCandidateColor"),
        })
    }

    /// 只读 classicui 的配置值（应用时合并改动与读回比对用）。
    pub fn classic_ui_map(&self) -> Result<ConfigMap, Fcitx5Error> {
        Ok(self.get_config(CLASSIC_UI_URI)?.0)
    }

    /// 写 classicui 配置（全量）；调用方负责之后 [`Self::reload_addon`] 并读回核对。
    pub fn set_classic_ui(&self, map: &ConfigMap) -> Result<(), Fcitx5Error> {
        self.call(
            "SetConfig",
            Some(&(CLASSIC_UI_URI.to_owned(), map.to_variant()).to_variant()),
        )?;
        Ok(())
    }

    /// 让一个插件重新加载自己的配置。
    pub fn reload_addon(&self, addon: &str) -> Result<(), Fcitx5Error> {
        self.call("ReloadAddonConfig", Some(&(addon,).to_variant()))?;
        Ok(())
    }

    /// `GetConfig`：回（配置值，字段描述原始 Variant——主题枚举选项从中解析）。
    fn get_config(&self, uri: &str) -> Result<(ConfigMap, glib::Variant), Fcitx5Error> {
        let reply = self.call("GetConfig", Some(&(uri,).to_variant()))?;
        let values = reply
            .child_value(0)
            .get::<glib::Variant>()
            .ok_or_else(|| Fcitx5Error::BadReply("GetConfig 的配置值不是变体".to_owned()))?;
        Ok((ConfigMap::from_variant(&values), reply.child_value(1)))
    }

    fn call(
        &self,
        method: &str,
        parameters: Option<&glib::Variant>,
    ) -> Result<glib::Variant, Fcitx5Error> {
        self.conn
            .call_sync(
                Some(BUS_NAME),
                OBJECT_PATH,
                INTERFACE,
                method,
                parameters,
                None,
                gio::DBusCallFlags::NONE,
                TIMEOUT_MS,
                None::<&gio::Cancellable>,
            )
            .map_err(|error| {
                // 调用期间 Fcitx5 退了也按「未运行」处理
                if error.matches(gio::DBusError::ServiceUnknown) {
                    Fcitx5Error::NotRunning
                } else {
                    Fcitx5Error::Bus(error)
                }
            })
    }
}
