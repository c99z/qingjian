//! 青简 Linux 设置应用：GTK4/libadwaita 原生界面，管「编辑草稿到保存生效」这一段。
//! 分三层：`state` 草稿与保存管线（不碰 GUI，可单测）、`adapters` 管理 socket 等外部接口、`ui` 控件。

pub mod adapters;
pub mod state;
pub mod ui;
