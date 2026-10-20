//! 最近提交驱动的词级重排，以及导航后禁止换序。

use std::time::Duration;

use qingjian_core::Engine;
use qingjian_core::sentence::SentenceScorer;
use qingjian_dictionary::Dictionary;
use qingjian_platform::protocol::{KeyEvent, KeyModifiers};

use super::{first, poll, press, router_with, tick_until_first, type_letters};

struct ContextModel;

impl SentenceScorer for ContextModel {
    fn score(&self, _: &str, _: &str, texts: &[&str]) -> Vec<f64> {
        vec![0.0; texts.len()]
    }

    fn supports_context(&self) -> bool {
        true
    }

    fn context_gain(&self, before: &str, texts: &[&str]) -> Vec<f64> {
        texts
            .iter()
            .map(|text| {
                if before.ends_with("明天") && *text == "再" {
                    8.0
                } else {
                    0.0
                }
            })
            .collect()
    }
}

#[test]
fn recent_commit_reorders_words_once_and_navigation_freezes_them() {
    for navigate in [false, true] {
        let mut engine = Engine::new(
            Dictionary::parse("明天\tming tian\t1000\n在\tzai\t200\n再\tzai\t100\n").unwrap(),
        );
        engine.set_context_ranking(true);
        engine.set_async_sentence_scorer(Some(Box::new(ContextModel)));
        let mut router = router_with(engine);
        assert_eq!(first(&type_letters(&mut router, "mingtian")), Some("明天"));
        press(
            &mut router,
            KeyEvent::new(0x20, Some(' '), KeyModifiers::default()),
        );
        assert_eq!(first(&type_letters(&mut router, "zai")), Some("在"));
        if navigate {
            press(
                &mut router,
                KeyEvent::new(0x28, None, KeyModifiers::default()),
            );
        }
        let frame = tick_until_first(&mut router, "再", Duration::from_millis(400));
        assert_eq!(first(&frame), Some(if navigate { "在" } else { "再" }));
        for _ in 0..3 {
            assert_eq!(first(&poll(&mut router)), first(&frame));
        }
    }
}
