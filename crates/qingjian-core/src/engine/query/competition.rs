//! 完整匹配词与整句共用路径评分，补回被词图切分或裁剪遗漏的整词。

use qingjian_dictionary::{SyllablePattern, canonical_syllable};

use crate::candidate::{Candidate, CandidateKind};
use crate::engine::{Engine, Learner};
use crate::parser::Segmentation;
use crate::ranking::{RECENT_CHOICE_BONUS, choice_bonus, weight_bonus};
use crate::sentence::{self, Context, Conversion, SentenceWord};

impl Engine {
    pub(super) fn compete_with_exact_word(
        &self,
        items: &[Candidate],
        best: &Segmentation,
        paths: &mut Vec<Conversion>,
    ) -> bool {
        if paths.is_empty() || best.syllables.iter().any(|s| !s.complete) {
            return false;
        }
        let letters: String = best
            .syllables
            .iter()
            .map(|s| canonical_syllable(&s.text))
            .collect();
        let Some(candidate) = items
            .iter()
            .find(|c| c.kind == CandidateKind::Chinese && c.syllables.concat() == letters)
        else {
            return false;
        };
        if paths.iter().any(|p| p.text == candidate.text) {
            // 已在词图里的整词也可能因个人偏好胜出；原来的拆词候选留在它后面供改选。
            return self.sentence_choice_bonus(&best.joined(""), &candidate.text) > 0.0
                && paths[0].text == candidate.text
                && paths[0].word_count() == 1;
        }
        let patterns: Vec<_> = candidate
            .syllables
            .iter()
            .map(|s| SyllablePattern::complete(s))
            .collect();
        let Some(frequency) = self
            .all_dictionaries()
            .into_iter()
            .flat_map(|d| d.lookup_exact(&patterns))
            .filter(|hit| hit.text == candidate.text)
            .map(|hit| hit.frequency)
            .max()
        else {
            return false;
        };
        let fallback =
            sentence::fallback_log_prob(frequency, (self.total_frequency() as f64).max(1.0).ln());
        // Viterbi 从句首起算；这里也从句首起算，不能混入词级排序所用的上屏上下文。
        let static_score = self
            .language_model
            .log_prob(None, &candidate.text)
            .unwrap_or(fallback);
        let score = sentence::transition_log_prob(
            &*self.language_model,
            self.personal(),
            Context::START,
            &candidate.text,
            fallback,
        ) + weight_bonus(self.learner.weight(&candidate.text))
            + self.sentence_choice_bonus(&best.joined(""), &candidate.text);
        paths.push(Conversion {
            text: candidate.text.clone(),
            syllables: candidate.syllables.clone(),
            words: vec![SentenceWord {
                text: candidate.text.clone(),
                syllables: candidate.syllables.clone(),
                placeholder: false,
            }],
            score,
            static_score,
            penalty: 0.0,
        });
        paths.sort_by(|a, b| b.score.total_cmp(&a.score));
        self.rescore_paths(paths, &best.joined(""));
        paths
            .first()
            .is_some_and(|p| p.text == candidate.text && p.word_count() == 1)
    }

    /// 同码改选的偏好只加在整段文本上，不能分摊给每个字或混入模型静态分。
    pub(super) fn sentence_choice_bonus(&self, input: &str, text: &str) -> f64 {
        choice_bonus(self.learner.choice_weight(input, text))
            + RECENT_CHOICE_BONUS * self.learner.recent_choice_weight(input, text)
    }
}
