//! 基础模型与知微分开评分；缓存无前文分，避免重复奖励常见词。

use std::collections::HashMap;
use std::sync::Mutex;

use qingjian_core::sentence::SentenceScorer;

use crate::{CharScorer, NeuralError};

pub struct ContextScorer {
    base: Box<dyn SentenceScorer>,

    context: CharScorer,

    empty: Mutex<HashMap<String, f64>>,
}

impl ContextScorer {
    pub fn new(base: Box<dyn SentenceScorer>, context: CharScorer) -> Result<Self, NeuralError> {
        if context.vocab().sep().is_some() {
            return Err(NeuralError::Corrupt("context model must be a character LM"));
        }
        Ok(Self {
            base,
            context,
            empty: Mutex::new(HashMap::new()),
        })
    }

    fn gains(&self, before: &str, texts: &[&str]) -> Result<Vec<f64>, NeuralError> {
        if before.is_empty() {
            return Ok(vec![0.0; texts.len()]);
        }
        // 不对未知字的 UNK 概率作语义判断，也不静默裁掉候选字符。
        let limit = self.context.model().config().context;
        if texts.iter().any(|text| {
            text.is_empty()
                || text.chars().count() >= limit
                || self
                    .context
                    .vocab()
                    .encode(text)
                    .contains(&crate::vocab::UNK)
        }) {
            return Ok(Vec::new());
        }
        let mut empty = self.empty.lock().unwrap_or_else(|e| e.into_inner());
        if empty.len() + texts.len() > 256 {
            empty.clear();
        }
        let missing: Vec<&str> = texts
            .iter()
            .copied()
            .filter(|text| !empty.contains_key(*text))
            .collect();
        for (text, score) in missing.iter().zip(self.context.score("", &missing)?) {
            empty.insert((*text).to_owned(), score);
        }
        let conditional = self.context.score(before, texts)?;
        Ok(texts
            .iter()
            .zip(conditional)
            .map(|(text, score)| (score - empty[*text]) / text.chars().count() as f64)
            .collect())
    }
}

impl SentenceScorer for ContextScorer {
    fn score(&self, context: &str, keys: &str, texts: &[&str]) -> Vec<f64> {
        self.base.score(context, keys, texts)
    }

    fn generate(&self, keys: &str, beam: usize, max_chars: usize) -> Vec<String> {
        self.base.generate(keys, beam, max_chars)
    }

    fn supports_context(&self) -> bool {
        true
    }

    fn context_gain(&self, before: &str, texts: &[&str]) -> Vec<f64> {
        match self.gains(before, texts) {
            Ok(gains) => gains,
            Err(error) => {
                tracing::warn!(%error, "语境评分失败，本轮保留基础排序");
                Vec::new()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ContextScorer;
    use crate::CharScorer;
    use qingjian_core::sentence::SentenceScorer;
    use std::path::Path;

    struct Base;

    impl SentenceScorer for Base {
        fn score(&self, _: &str, _: &str, texts: &[&str]) -> Vec<f64> {
            vec![-7.0; texts.len()]
        }
    }

    #[test]
    fn real_model_gain_matches_conditional_minus_empty_and_keeps_base_separate() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/models/hanzhang-zhiwei/hanzhang-zhiwei-small.qjm");
        if !path.exists() {
            eprintln!("缺少真实知微模型，此项未验证");
            return;
        }
        let lm = CharScorer::load(&path).unwrap();
        let texts = ["接口", "借口"];
        let conditional = lm.score("这个程序需要调用网络", &texts).unwrap();
        let empty = lm.score("", &texts).unwrap();
        let scorer = ContextScorer::new(Box::new(Base), lm).unwrap();
        let gains = scorer.context_gain("这个程序需要调用网络", &texts);
        assert_eq!(scorer.score("前文", "jiekou", &texts), [-7.0, -7.0]);
        for i in 0..texts.len() {
            assert!((gains[i] - (conditional[i] - empty[i]) / 2.0).abs() < 0.001);
        }
        assert!(gains[0] > gains[1]);
        assert_eq!(scorer.context_gain("", &texts), [0.0, 0.0]);
        assert_eq!(scorer.context_gain("这个程序需要调用网络", &texts), gains);
        assert!(scorer.context_gain("前文", &["🦄", "接口"]).is_empty());
        assert!(scorer.context_gain("前文", &[&"字".repeat(128)]).is_empty());
        assert!(
            scorer
                .context_gain("🙂\nAPI调用", &texts)
                .iter()
                .all(|s| s.is_finite())
        );
    }
}
