//! 草稿校验：保存前把非法字段逐个报出来（字段路径 + 中文说明），而不是落盘后由运行时静默回退。
//! 判定复用各分节现有的回退逻辑做标准——运行时会退回缺省的写法，这里报成错误；运行时行为不变。

use qingjian_predict::PredictConfig;

use crate::config::{Config, GeneralConfig, MAX_PAGE_SIZE, Scheme, ShortcutConfig};

use super::field_error::FieldError;

/// 请求超时再低，正常网络也来不及返回。
const MIN_TIMEOUT_MS: u64 = 500;

/// 超时上限：一分钟还没返回的请求没有等的价值。
const MAX_TIMEOUT_MS: u64 = 60_000;

/// 防抖上限：再长就感觉像没反应。
const MAX_DEBOUNCE_MS: u64 = 5_000;

/// 校验整份草稿配置，返回全部字段错误（空 = 可以保存）。
pub fn validate(draft: &Config) -> Vec<FieldError> {
    let mut errors = Vec::new();
    general(&draft.general, &mut errors);
    shortcut(&draft.shortcut, &mut errors);
    predict(&draft.predict, &mut errors);
    if let Err(message) = qingjian_core::custom_phrase::validate_phrases(&draft.custom_phrases) {
        errors.push(FieldError::new("custom_phrases", message));
    }
    errors
}

fn general(general: &GeneralConfig, errors: &mut Vec<FieldError>) {
    if general.page_size() != general.page_size {
        errors.push(FieldError::new(
            "general.page_size",
            format!("每页候选数要在 1–{MAX_PAGE_SIZE} 之间"),
        ));
    }
    // 合法写法经 page_keys() 原样返回；回退过就说明写法不合法
    let (previous, next) = general.page_keys();
    if general.page_keys != format!("{previous}{next}") {
        errors.push(FieldError::new(
            "general.page_keys",
            "翻页键要是两个不同的可见 ASCII 字符，且不能是字母或数字",
        ));
    }
    let scheme = general.scheme.trim();
    if !scheme.is_empty() && scheme.parse::<Scheme>().is_err() {
        errors.push(FieldError::new(
            "general.scheme",
            format!("不认识的拼音方案「{scheme}」"),
        ));
    }
}

fn shortcut(shortcut: &ShortcutConfig, errors: &mut Vec<FieldError>) {
    // 与 qingjian_core::ModeKeys::CANDIDATES 一致：这三个字母不是任何拼音音节的开头
    let candidates = ['v', 'u', 'i'];
    let mode = shortcut.mode;
    if !candidates.contains(&mode.expression) {
        errors.push(FieldError::new(
            "shortcut.expression",
            "表达式模式键只能是 v / u / i",
        ));
    }
    if !candidates.contains(&mode.question) {
        errors.push(FieldError::new(
            "shortcut.question",
            "问字模式键只能是 v / u / i",
        ));
    }
    if candidates.contains(&mode.expression)
        && candidates.contains(&mode.question)
        && mode.expression == mode.question
    {
        errors.push(FieldError::new(
            "shortcut.question",
            "表达式与问字模式键不能相同",
        ));
    }
    if shortcut.translation.is_empty() {
        errors.push(FieldError::new(
            "shortcut.translation",
            "第一条译词的修饰键不能为空",
        ));
    }
    if shortcut.translation_second.is_empty() {
        errors.push(FieldError::new(
            "shortcut.translation_second",
            "第二条译词的修饰键不能为空",
        ));
    }
    if !shortcut.translation.is_empty()
        && !shortcut.translation_second.is_empty()
        && shortcut.translation == shortcut.translation_second
    {
        errors.push(FieldError::new(
            "shortcut.translation_second",
            "两条译词的修饰键不能相同",
        ));
    }
    if shortcut.delete_candidate.is_empty() {
        errors.push(FieldError::new(
            "shortcut.delete_candidate",
            "删候选的修饰键不能为空",
        ));
    } else if shortcut.delete_candidate == shortcut.translation
        || shortcut.delete_candidate == shortcut.translation_second
    {
        errors.push(FieldError::new(
            "shortcut.delete_candidate",
            "删候选的修饰键不能与译词键相同",
        ));
    }
}

fn predict(predict: &PredictConfig, errors: &mut Vec<FieldError>) {
    if !predict.valid_endpoint() {
        errors.push(FieldError::new(
            "predict.base_url",
            "API URL 须为完整的 http/https 基础地址，不能包含密钥、查询参数或片段",
        ));
    }
    if predict.model.trim().is_empty() {
        errors.push(FieldError::new("predict.model", "模型名称不能为空"));
    }
    if !(MIN_TIMEOUT_MS..=MAX_TIMEOUT_MS).contains(&predict.timeout_ms) {
        errors.push(FieldError::new(
            "predict.timeout_ms",
            format!("请求超时要在 {MIN_TIMEOUT_MS}–{MAX_TIMEOUT_MS} 毫秒之间"),
        ));
    }
    if predict.debounce_ms > MAX_DEBOUNCE_MS {
        errors.push(FieldError::new(
            "predict.debounce_ms",
            format!("防抖不能超过 {MAX_DEBOUNCE_MS} 毫秒"),
        ));
    }
    if predict.slots > MAX_PAGE_SIZE {
        errors.push(FieldError::new(
            "predict.slots",
            format!("云候选格数最多 {MAX_PAGE_SIZE}（一页候选的格数）"),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_config_passes() {
        assert!(validate(&Config::default()).is_empty());
    }

    #[test]
    fn legacy_scheme_spellings_are_accepted() {
        let mut draft = Config::default();
        for spelling in ["pinyin", "xiaohe", "zhuyin", "none", "wubi86", ""] {
            draft.general.scheme = spelling.to_owned();
            assert!(validate(&draft).is_empty(), "{spelling}");
        }
    }
}
