//! 后台快照、API 请求与词库生成；不触碰 Engine 或正式学习表。

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use qingjian_learning::organization::{OrganizationDraft, read_evidence};
use qingjian_predict::{PredictConfig, organize_words};

pub(super) fn run(
    log: &Path,
    base: &Path,
    api: &PredictConfig,
    hint: &str,
    cancel: &AtomicBool,
) -> Result<(OrganizationDraft, Vec<String>), String> {
    let dictionary =
        qingjian_dictionary::Dictionary::from_path(base).map_err(|_| "无法读取基础词库")?;
    let evidence = read_evidence(log, &dictionary).map_err(|_| "无法读取输入日志快照")?;
    if evidence.is_empty() {
        return Err("暂无跨两天且重复三次的有效词；未调用 API".into());
    }
    if cancel.load(Ordering::Acquire) {
        return Err("整理已取消".into());
    }
    let reply = organize_words(
        api,
        &serde_json::to_value(&evidence).map_err(|_| "整理证据编码失败")?,
        hint,
    )?;
    if cancel.load(Ordering::Acquire) {
        return Err("整理已取消".into());
    }
    let draft = OrganizationDraft::build(&evidence, &reply.accepted, &dictionary)?;
    Ok((draft, reply.domains))
}
