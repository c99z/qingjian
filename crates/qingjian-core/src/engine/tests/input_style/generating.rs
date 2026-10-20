//! 同步、缓存与晚到的后台生成结果都按当前中文策略筛选。

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use qingjian_dictionary::Dictionary;

use crate::sentence::SentenceScorer;
use crate::{ChineseInputStyle, Engine};

struct Generator(Arc<AtomicUsize>);

impl SentenceScorer for Generator {
    fn score(&self, _: &str, _: &str, texts: &[&str]) -> Vec<f64> {
        vec![-1.0; texts.len()]
    }

    fn generate(&self, _: &str, _: usize, _: usize) -> Vec<String> {
        self.0.fetch_add(1, Ordering::SeqCst);
        ["候选C", "候选Ｃ", "候选词", "候选2"]
            .map(str::to_owned)
            .to_vec()
    }
}

#[test]
fn generated_results_follow_current_style_even_when_requested_before_switching() {
    for asynchronous in [false, true] {
        let calls = Arc::new(AtomicUsize::new(0));
        let engine =
            Engine::new(Dictionary::parse("候选\thou xuan\t1000\n后\thou\t100\n").unwrap());
        let scorer = Box::new(Generator(calls.clone()));
        let mut engine = if asynchronous {
            engine.with_async_sentence_scorer(scorer, None, None, None)
        } else {
            engine.with_sentence_scorer(scorer, None, None, None)
        };
        engine.set_input("houxuanc");
        engine.query().unwrap();
        if asynchronous {
            assert!(engine.request_rescoring());
        }
        engine.set_chinese_input_style(ChineseInputStyle::Chinese);
        if asynchronous {
            let start = Instant::now();
            while !engine.poll_rescoring() {
                assert!(start.elapsed() < Duration::from_secs(3));
                std::thread::sleep(Duration::from_millis(1));
            }
        }
        let items = engine.query().unwrap().candidates.items;
        assert_eq!(items[0].text, "候选词");
        assert!(items.iter().any(|c| c.text == "候选2"));
        assert!(
            items
                .iter()
                .all(|c| c.text != "候选C" && c.text != "候选Ｃ")
        );
        // 缓存保留原始结果，来回切换不会丢失候选或重复推理。
        engine.set_chinese_input_style(ChineseInputStyle::Mixed);
        assert_eq!(engine.query().unwrap().candidates.items[0].text, "候选C");
        engine.set_chinese_input_style(ChineseInputStyle::Chinese);
        assert_eq!(engine.query().unwrap().candidates.items[0].text, "候选词");
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
}
