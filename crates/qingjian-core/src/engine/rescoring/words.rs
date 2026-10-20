//! 词级语境融合：只重排同覆盖、同读音与同匹配结构的前六项。

use crate::engine::Engine;
use crate::ranking::{self, Scored};

impl Engine {
    pub(in crate::engine) fn rescore_words(
        &self,
        items: &mut [Scored<'_>],
        context_score: impl Fn(&Scored<'_>) -> (u32, f64),
    ) {
        if !self.context_ranking_enabled() || !self.has_context_scorer() {
            return;
        }
        let Some(first) = items.first() else { return };
        let count = items
            .iter()
            .take(6)
            .take_while(|item| {
                item.hit.exact == first.hit.exact
                    && item.coverage == first.coverage
                    && item.abbreviated == first.abbreviated
                    && item.full_last == first.full_last
                    && item.hit.pinyin == first.hit.pinyin
                    && item
                        .hit
                        .text
                        .chars()
                        .all(|c| ('\u{3400}'..='\u{9fff}').contains(&c))
            })
            .count();
        if count < 2 {
            return;
        }
        let mut state = self.context_ranking.borrow_mut();
        state.collect(items[..count].iter().map(|item| item.hit.text));
        let incumbent = items[0].hit.text;
        let score = |item: &Scored<'_>| {
            let (choice, log_prob) = context_score(item);
            log_prob + ranking::choice_bonus(choice) + ranking::weight_bonus(item.weight)
                - item.penalty
                + state.bonus(item.hit.text)
                + if item.hit.text == incumbent { 0.3 } else { 0.0 }
        };
        items[..count].sort_by(|a, b| score(b).total_cmp(&score(a)));
    }

    pub(in crate::engine) fn finish_sync_context(&self) -> bool {
        let Some(scorer) = &self.sentence_scorer else {
            return false;
        };
        let mut state = self.context_ranking.borrow_mut();
        let texts = state.take();
        if texts.is_empty() {
            return false;
        }
        let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
        let gains = scorer.context_gain(&state.before, &refs);
        state.accept(&texts, &gains);
        true
    }
}
