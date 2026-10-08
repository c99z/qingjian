//! 保存管线：校验 → 落盘 → 通知服务重载，汇总成给用户看的结果。
//! 纯同步 IO，GUI 在后台线程里调；[`SaveOutcome`] 区分「没保存 / 冲突 / 部分成功 / 全部生效」，界面照实展示。

use std::path::Path;

use qingjian_platform::settings::{
    self, ApplyError, ApplyOutcome, ComponentStatus, ConfigPatch, FieldConflict, FieldError,
    SettingsSnapshot,
};

use crate::adapters::admin::{AdminClient, AdminError};

/// 一次「应用」的结果。
#[derive(Debug)]
pub enum SaveOutcome {
    /// 草稿拼不出完整配置（控件层一般已拦住，比如非法的修饰键写法不进入草稿）。
    DraftBroken(String),

    /// 校验未过：逐字段错误，什么都没写。
    Invalid(Vec<FieldError>),

    /// 与外部修改冲突，整体没写。
    Conflicts(Vec<FieldConflict>),

    /// 写入本身失败（读盘、文件损坏、写盘）。
    WriteFailed(String),

    /// 已写入。`server` 是通知服务重载的结果：服务没跑是 [`ComponentStatus::Saved`]
    /// （「下次启动生效」），重载失败是 `Failed`——配置已落盘但服务没应用，部分成功要如实展示。
    Saved { server: ComponentStatus },
}

/// 跑完整保存流程。`snapshot` / `patch` 传克隆，本函数不改调用方的草稿状态；
/// 返回 `Saved` 后由调用方推进基准（[`crate::state::DraftStore::mark_saved`]）。
pub fn save(
    path: &Path,
    snapshot: &SettingsSnapshot,
    patch: &ConfigPatch,
    admin: &AdminClient,
) -> SaveOutcome {
    let draft = match patch.draft(snapshot) {
        Ok(draft) => draft,
        Err(error) => return SaveOutcome::DraftBroken(error),
    };
    let errors = settings::validate(&draft);
    if !errors.is_empty() {
        return SaveOutcome::Invalid(errors);
    }
    let outcome = match settings::apply(path, snapshot, patch) {
        Ok(outcome) => outcome,
        Err(error) => return SaveOutcome::WriteFailed(describe_apply_error(&error)),
    };
    match outcome {
        ApplyOutcome::Conflicts(conflicts) => SaveOutcome::Conflicts(conflicts),
        ApplyOutcome::Applied => {
            let server = match admin.reload() {
                Ok(status) => status,
                Err(AdminError::NotRunning) => ComponentStatus::Saved,
                Err(error) => ComponentStatus::Failed(format!(
                    "配置已保存，通知服务重载失败：{}",
                    error.user_message()
                )),
            };
            SaveOutcome::Saved { server }
        }
    }
}

/// `ApplyError` 的 Display 是英文（日志用），给用户换成中文说明。
fn describe_apply_error(error: &ApplyError) -> String {
    match error {
        ApplyError::Read { source, .. } => format!("读取配置文件失败：{source}"),
        ApplyError::Broken { message, .. } => {
            format!("配置文件有语法错误，为避免丢失内容未写入；请手动修正后重试（{message}）")
        }
        ApplyError::Phrases(message) => format!("自定义短语不合法：{message}"),
        ApplyError::Write { source, .. } => format!("写入配置文件失败：{source}"),
    }
}

#[cfg(test)]
mod tests;
