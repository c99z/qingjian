//! 草稿与保存管线：设置窗口的非 GUI 状态。
//! [`DraftStore`] 管「快照 + 草稿修改」的累积、撤销与基准推进；[`save`] 管「校验 → 落盘 → 通知重载」的顺序。

mod draft;
mod save;

pub use draft::DraftStore;
pub use save::{SaveOutcome, save};
