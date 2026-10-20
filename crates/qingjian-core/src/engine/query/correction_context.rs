//! 纠错竞争时用光标前文或最近上屏词补充句首概率，避免把所有输入都当作新句子。

use crate::engine::{Engine, RESCORE_CONTEXT_CHARS, take_last_chars};
use crate::sentence::{self, Context, Conversion};

/// 短前文只能提供有限证据，不能覆盖用户的明确改选。
const CONTEXT_LIMIT: f64 = 4.0;

impl Engine {
    pub(in crate::engine) fn has_typo_paths(&self, paths: &[Conversion], keys: &str) -> bool {
        self.pinyin_correction
            && paths.iter().any(|path| {
                if !path.altered() {
                    return false;
                }
                let alignment = self.align(keys, &path.syllables);
                alignment.consumed == keys.len() && !alignment.typos.is_empty()
            })
    }

    pub(super) fn apply_correction_context(&self, paths: &mut [Conversion], keys: &str) {
        if !self.has_typo_paths(paths, keys) {
            return;
        }
        // 应用提供了前文时以应用为准；空前文或句末标点不能偷偷接上本地历史。
        let clauses = self.rescoring_before.as_deref().and_then(|before| {
            if before.chars().last().is_none_or(|c| !sentence::is_han(c)) {
                return None;
            }
            sentence::segment_text(
                &take_last_chars(before, RESCORE_CONTEXT_CHARS),
                &*self.language_model,
            )
        });
        let words = clauses.as_ref().and_then(|c| c.last());
        let context = if self.rescoring_before.is_some() {
            Context {
                previous: words.and_then(|w| w.last()).map(String::as_str),
                earlier: words.and_then(|w| w.len().checked_sub(2).map(|i| w[i].as_str())),
            }
        } else {
            self.chain.context()
        };
        if context.previous.is_none() {
            return;
        }
        for path in paths {
            let Some(first) = path.words.first() else {
                continue;
            };
            let Some(base) = self.language_model.log_prob(None, &first.text) else {
                continue;
            };
            let contextual = sentence::transition_log_prob(
                &*self.language_model,
                self.personal(),
                context,
                &first.text,
                base,
            );
            let original = self.personal().blend(Context::START, &first.text, base);
            path.score += (contextual - original).clamp(-CONTEXT_LIMIT, CONTEXT_LIMIT);
        }
    }
}
