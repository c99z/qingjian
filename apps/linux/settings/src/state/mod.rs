//! 草稿与保存管线：设置窗口的非 GUI 状态。
//! [`DraftStore`] 管「快照 + 草稿修改」的累积、撤销与基准推进（青简配置补丁与 Fcitx5 外观草稿并列）；
//! [`save`] 管「校验 → 落盘 → 应用外观 → 通知重载」的顺序，两段结果分别报告。

mod appearance;
mod credential;
mod draft;
mod save;

pub use appearance::AppearanceDraft;
pub use credential::CredentialEdit;
pub use draft::DraftStore;
pub use save::{SaveOutcome, save, save_with_credential};
