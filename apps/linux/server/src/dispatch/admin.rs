//! 管理请求的分派与报告组装；管理操作与按键处理在主线程串行，只暴露协议枚举里的能力。
use super::Router;
use crate::protocol::admin::{
    AdminCommand, AdminResponse, ConfigStatus, LINUX_ADMIN_PROTOCOL, ModelStatus, StatsReport,
    StatusReport,
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
            AdminCommand::Backup => match self.create_backup() {
                Ok(path) => AdminResponse::Backup(path),
                Err(message) => AdminResponse::Error(message),
            },
            AdminCommand::Restore { transfer } => {
                AdminResponse::Maintenance(self.restore_backup(&transfer))
            }
            AdminCommand::ClearInputLog => AdminResponse::Maintenance(self.clear_input_log()),
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
            cloud: self.cloud.status.clone(),
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

impl Router {
    fn clear_input_log(&mut self) -> qingjian_platform::settings::ComponentStatus {
        use qingjian_core::{InputLogger, NoInputLogger};
        use qingjian_learning::InputLog;
        use qingjian_platform::settings::ComponentStatus;
        let Some(path) = self.reload.as_ref().and_then(|r| r.dirs.input_log.clone()) else {
            return ComponentStatus::Failed("服务没有输入日志路径".to_owned());
        };
        let enabled = self.reload.as_ref().is_some_and(|r| r.applied_input_log);
        // 先 flush 并销毁旧 BufWriter，再截断；旧缓冲不能在本次清空后写回。
        self.engine.input_logger_mut().flush();
        self.engine.set_input_logger(Box::new(NoInputLogger));
        let cleared = match std::fs::symlink_metadata(&path) {
            Ok(meta) if meta.is_file() => InputLog::clear(&path),
            Ok(_) => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "input log is not a regular file",
            )),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        };
        let mut reopened = true;
        if enabled {
            let log = InputLog::open(&path);
            reopened = log.is_enabled();
            self.engine.set_input_logger(Box::new(log));
        }
        match cleared {
            Err(error) => ComponentStatus::Failed(format!("清空输入日志失败：{error}")),
            Ok(()) if !reopened => ComponentStatus::Failed(
                "日志已清空，但写入端无法重新打开，请检查目录权限".to_owned(),
            ),
            Ok(()) => ComponentStatus::Applied,
        }
    }
}
