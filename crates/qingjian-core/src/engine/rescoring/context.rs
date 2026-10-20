//! 每轮查询的有限语境候选池；身份代次防止 A→B→A 接受旧结果。

use std::collections::HashMap;

#[derive(Default)]
pub(crate) struct ContextRanking {
    pub enabled: bool,

    pub epoch: u64,

    pub before: String,

    keys: String,

    texts: Vec<String>,

    gains: HashMap<String, f64>,

    submitted: bool,
}

impl ContextRanking {
    pub fn invalidate(&mut self) {
        self.epoch = self.epoch.wrapping_add(1);
        self.texts.clear();
        self.gains.clear();
        self.submitted = false;
    }

    pub fn prepare(&mut self, before: String, keys: &str) {
        if self.before != before || self.keys != keys {
            self.invalidate();
            self.before = before;
            self.keys = keys.to_owned();
        }
    }

    pub fn collect<'a>(&mut self, texts: impl Iterator<Item = &'a str>) {
        if !self.enabled || self.before.is_empty() || self.submitted {
            return;
        }
        for text in texts {
            if self.texts.len() >= 12 {
                break;
            }
            if !text.is_empty()
                && text.chars().all(|c| ('\u{3400}'..='\u{9fff}').contains(&c))
                && !self.texts.iter().any(|t| t == text)
            {
                self.texts.push(text.to_owned());
            }
        }
    }

    pub fn pending(&self) -> bool {
        self.enabled && !self.submitted && self.texts.len() > 1
    }

    pub fn take(&mut self) -> Vec<String> {
        if !self.pending() {
            return Vec::new();
        }
        self.submitted = true;
        self.texts.clone()
    }

    pub fn accept(&mut self, texts: &[String], gains: &[f64]) {
        if texts != self.texts || gains.len() != texts.len() || gains.iter().any(|x| !x.is_finite())
        {
            return;
        }
        // 以同一池均值居中，每字增益限幅；最多 ±1.5 nat，不能无限覆盖个人选择。
        let mean = gains.iter().sum::<f64>() / gains.len().max(1) as f64;
        self.gains = texts
            .iter()
            .cloned()
            .zip(
                gains
                    .iter()
                    .map(|gain| 0.5 * (gain - mean).clamp(-3.0, 3.0)),
            )
            .collect();
    }

    pub fn has_gains(&self) -> bool {
        self.enabled && !self.gains.is_empty()
    }

    pub fn bonus(&self, text: &str) -> f64 {
        if self.enabled {
            self.gains.get(text).copied().unwrap_or(0.0)
        } else {
            0.0
        }
    }
}
