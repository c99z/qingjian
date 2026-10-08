//! Fcitx5 经典面板的 D-Bus 适配：配置读写、面板检测、主题复制与读回核对。
//! 同步阻塞 API（后台线程里调），与 GTK 无关；字段映射、主题复制等纯逻辑各有单测。

mod appearance;
mod apply;
mod client;
mod error;
mod font;
mod probe;
mod snapshot;
mod theme;
mod value;

pub mod color;

pub use appearance::{AppearanceValues, ApplyPlan, FieldMismatch, preflight};
pub use apply::apply;
pub use client::{CLASSIC_UI_URI, Fcitx5Client};
pub use error::Fcitx5Error;
pub use font::{FontDesc, FontWeight, fonts_match};
pub use probe::{ClassicUiPanel, PanelProbe};
pub use snapshot::ClassicUiState;
pub use theme::{CUSTOM_THEME_NAME, CommentStyle, InstalledTheme, ThemeOption, theme_options};
pub use value::{ConfigMap, ConfigValue};
