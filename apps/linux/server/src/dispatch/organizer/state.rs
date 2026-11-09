//! 夜间整理的单任务状态；配置变化取消旧任务，结果只在安全的组句边界应用。

use std::sync::{Arc, atomic::AtomicBool, mpsc::Receiver};
use std::time::Instant;

use qingjian_learning::organization::OrganizationDraft;
use qingjian_platform::OrganizerConfig;
use qingjian_predict::PredictConfig;

type RunResult = Result<(OrganizationDraft, Vec<String>), String>;

#[derive(Default)]
pub(crate) struct OrganizerState {
    pub settings: Option<(OrganizerConfig, PredictConfig, bool)>,

    pub last_day: Option<String>,

    pub initialized: bool,

    pub last_check: Option<Instant>,

    pub cancel: Arc<AtomicBool>,

    pub running: Option<Receiver<RunResult>>,

    pub pending: Option<(OrganizationDraft, Vec<String>)>,
}
