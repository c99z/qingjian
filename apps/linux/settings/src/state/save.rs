//! 保存管线：校验 → 落盘 → 通知服务重载，汇总成给用户看的结果。
//! 纯同步 IO，GUI 在后台线程里调；[`SaveOutcome`] 区分「没保存 / 冲突 / 部分成功 / 全部生效」，界面照实展示。

use std::path::Path;

use qingjian_platform::settings::{
    self, ApplyError, ApplyOutcome, ComponentStatus, ConfigPatch, FieldConflict, FieldError,
    SettingsSnapshot,
};

use crate::adapters::admin::{AdminClient, AdminError};
use crate::adapters::fcitx5::{self, Fcitx5Client, Fcitx5Error};

use super::appearance::AppearanceDraft;

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
    /// `appearance` 是 Fcitx5 外观的应用结果：`None` 表示外观草稿本来就是空的；
    /// `Failed` 时外观草稿保留，不能显示成全部成功。
    Saved {
        server: ComponentStatus,
        appearance: Option<ComponentStatus>,
    },
}

/// 跑完整保存流程。`snapshot` / `patch` / `appearance` 传克隆，本函数不改调用方的草稿状态；
/// 返回 `Saved` 后由调用方推进基准（[`crate::state::DraftStore::mark_saved`]）。
pub fn save(
    path: &Path,
    snapshot: &SettingsSnapshot,
    patch: &ConfigPatch,
    admin: &AdminClient,
    appearance: Option<&AppearanceDraft>,
) -> SaveOutcome {
    save_with_credential(path, snapshot, patch, admin, appearance, None)
}

/// 凭据与配置一同应用，失败时恢复原凭据；密钥不进入错误详情。
pub fn save_with_credential(
    path: &Path,
    snapshot: &SettingsSnapshot,
    patch: &ConfigPatch,
    admin: &AdminClient,
    appearance: Option<&AppearanceDraft>,
    credential: Option<&super::credential::CredentialEdit>,
) -> SaveOutcome {
    let credential_path = path.with_file_name(".env");
    let previous = if credential.is_some() {
        match std::fs::read(&credential_path) {
            Ok(bytes) => Some(bytes),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(_) => return SaveOutcome::WriteFailed("无法读取原凭据，未保存".into()),
        }
    } else {
        None
    };
    if let Some(edit) = credential {
        match &edit.base {
            Err(message) => return SaveOutcome::WriteFailed(message.clone()),
            Ok(base) if base.as_deref().map(str::as_bytes) != previous.as_deref() => {
                return SaveOutcome::WriteFailed(
                    "本机凭据在编辑期间被其他程序修改，请撤销并重新打开设置后再编辑密钥".into(),
                );
            }
            _ => {}
        }
    }
    let draft = match patch.draft(snapshot) {
        Ok(draft) => draft,
        Err(error) => return SaveOutcome::DraftBroken(error),
    };
    let errors = settings::validate(&draft);
    if !errors.is_empty() {
        return SaveOutcome::Invalid(errors);
    }
    if let Some(key) = credential
        && let Err(message) = qingjian_platform::credentials::write_key(
            &credential_path,
            &draft.predict.api_key_env,
            &key.key,
        )
    {
        return SaveOutcome::WriteFailed(message);
    }
    let outcome = save_config(path, snapshot, patch, admin, appearance);
    if credential.is_some() && !matches!(outcome, SaveOutcome::Saved { .. }) {
        let restored = match previous {
            Some(bytes) => qingjian_platform::credentials::write_private(&credential_path, &bytes),
            None => std::fs::remove_file(&credential_path),
        };
        if restored.is_err() {
            return SaveOutcome::WriteFailed("设置未保存，且原凭据恢复失败，请检查 .env".into());
        }
    }
    outcome
}

fn save_config(
    path: &Path,
    snapshot: &SettingsSnapshot,
    patch: &ConfigPatch,
    admin: &AdminClient,
    appearance: Option<&AppearanceDraft>,
) -> SaveOutcome {
    let draft = match patch.draft(snapshot) {
        Ok(draft) => draft,
        Err(error) => return SaveOutcome::DraftBroken(error),
    };
    let errors = settings::validate(&draft);
    if !errors.is_empty() {
        return SaveOutcome::Invalid(errors);
    }
    if let Some(appearance) = appearance
        && let Some(base) = appearance.base()
    {
        let result = Fcitx5Client::connect()
            .and_then(|client| fcitx5::preflight(&client, base, &appearance.values()));
        if let Err(error) = result
            && !matches!(error, Fcitx5Error::NotRunning)
        {
            return SaveOutcome::WriteFailed(error.user_message());
        }
    }
    let outcome = match settings::apply(path, snapshot, patch) {
        Ok(outcome) => outcome,
        Err(error) => return SaveOutcome::WriteFailed(describe_apply_error(&error)),
    };
    match outcome {
        ApplyOutcome::Conflicts(conflicts) => SaveOutcome::Conflicts(conflicts),
        ApplyOutcome::Applied => {
            // 顺序按 2.3：写入青简配置 → Fcitx5 应用外观 → 通知服务重载
            let appearance = appearance.map(apply_appearance);
            let server = match admin.reload() {
                Ok(status) => status,
                Err(AdminError::NotRunning) => ComponentStatus::Saved,
                Err(error) => ComponentStatus::Failed(format!(
                    "配置已保存，通知服务重载失败：{}",
                    error.user_message()
                )),
            };
            SaveOutcome::Saved { server, appearance }
        }
    }
}

/// 应用外观草稿：Fcitx5 没跑（探测时就没基准，或保存时掉线）都报失败并保留草稿。
fn apply_appearance(draft: &AppearanceDraft) -> ComponentStatus {
    let Some(base) = draft.base() else {
        return ComponentStatus::Failed(Fcitx5Error::NotRunning.user_message());
    };
    match Fcitx5Client::connect().and_then(|client| fcitx5::apply(&client, base, &draft.values())) {
        Ok(()) => ComponentStatus::Applied,
        Err(error) => ComponentStatus::Failed(error.user_message()),
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
