//! 管理请求的分派与报告组装；管理操作与按键处理在主线程串行，只暴露协议枚举里的能力。
use super::Router;
use crate::protocol::admin::{
    AdminCommand, AdminResponse, CloudStatus, ConfigStatus, LINUX_ADMIN_PROTOCOL, ModelStatus,
    StatsReport, StatusReport,
};
use serde_json::Value;

impl Router {
    /// 一条管理请求；版本校验在连接线程做过了，这里只按命令执行。
    pub fn handle_admin(&mut self, value: Value) -> Option<Value> {
        let command: AdminCommand = serde_json::from_value(value.get("Admin")?.clone()).ok()?;
        let response = match command {
            AdminCommand::Status => AdminResponse::Status(self.status_report()),
            AdminCommand::Reload => AdminResponse::Reload(self.reload_now()),
            AdminCommand::Stats => AdminResponse::Stats(self.stats_report()),
        };
        serde_json::to_value(response).ok()
    }

    fn status_report(&self) -> StatusReport {
        let (pending, last_error) = self
            .reload
            .as_ref()
            .map(|reload| (reload.pending.is_some(), reload.last_error.clone()))
            .unwrap_or_default();
        StatusReport {
            version: env!("CARGO_PKG_VERSION").to_owned(),
            protocol: LINUX_ADMIN_PROTOCOL,
            config: ConfigStatus {
                generation: self.reload_generation(),
                pending,
                last_error,
            },
            model: self.model_status(),
            cloud: CloudStatus::Disabled,
        }
    }

    fn model_status(&self) -> ModelStatus {
        if self.model_loader.is_some() {
            return ModelStatus::Loading;
        }
        if self.engine.has_sentence_scorer() {
            return ModelStatus::Loaded;
        }
        let enabled = self
            .reload
            .as_ref()
            .is_none_or(|reload| reload.applied_model);
        if !enabled {
            return ModelStatus::Disabled;
        }
        ModelStatus::Failed(
            self.model_error
                .clone()
                .unwrap_or_else(|| "没有本地整句模型文件".to_owned()),
        )
    }

    fn stats_report(&self) -> StatsReport {
        StatsReport {
            usage: self.engine.usage_summary().into(),
            vocabulary: self.engine.vocabulary_summary().into(),
        }
    }
}
