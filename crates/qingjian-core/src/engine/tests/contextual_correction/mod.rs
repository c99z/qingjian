//! 纠错候选不被逐字偏好的模型抹掉，语境和改选仍可改变结果。

mod literal;
mod switches;

use std::time::{Duration, Instant};

use qingjian_dictionary::Dictionary;

use crate::Engine;
use crate::engine::tests::{CountingLearner, WordLearner};
use crate::sentence::{LanguageModel, SentenceScorer};

struct Model;

impl LanguageModel for Model {
    fn log_prob(&self, previous: Option<&str>, word: &str) -> Option<f64> {
        Some(match (previous, word) {
            (Some("衣服"), "没") => -0.1,
            (Some("衣服"), "没关系") => -20.0,
            (_, "没关系") => -8.0,
            (_, "没") => -4.0,
            (_, "敢") => -7.0,
            (_, "洗") => -8.0,
            (_, "衣服") => -2.0,
            _ => return None,
        })
    }
}

struct LiteralScorer;

impl SentenceScorer for LiteralScorer {
    fn score(&self, _: &str, _: &str, texts: &[&str]) -> Vec<f64> {
        texts
            .iter()
            .map(|t| if *t == "没敢洗" { -0.1 } else { -30.0 })
            .collect()
    }
}

fn engine(asynchronous: bool) -> Engine {
    let engine = Engine::new(Dictionary::parse(
        "没关系\tmei guan xi\t800000\n没\tmei\t900000\n敢\tgan\t200000\n洗\txi\t100000\n衣服\tyi fu\t1000\n"
    ).unwrap())
        .with_language_model(Box::new(Model))
        .with_learner(Box::new(CountingLearner(Default::default())));
    if asynchronous {
        engine.with_async_sentence_scorer(Box::new(LiteralScorer), Some(1.0), None, None)
    } else {
        engine.with_sentence_scorer(Box::new(LiteralScorer), Some(1.0), None, None)
    }
}

fn settle(engine: &mut Engine) {
    let start = Instant::now();
    loop {
        engine.query().unwrap();
        if !engine.request_rescoring() {
            break;
        }
        while !engine.poll_rescoring() {
            assert!(start.elapsed() < Duration::from_secs(3));
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

#[test]
fn strong_correction_survives_neural_update_and_keeps_literal_alternative() {
    for asynchronous in [false, true] {
        let mut engine = engine(asynchronous);
        engine.set_input("meiganxi");
        assert_eq!(engine.query().unwrap().candidates.items[0].text, "没关系");
        settle(&mut engine);
        let items = engine.query().unwrap().candidates.items;
        assert_eq!(items[0].text, "没关系");
        assert!(items.iter().any(|c| c.text == "没敢洗"));
        assert_eq!(engine.commit(&items[0]), "没关系");
        assert!(engine.composition().is_empty());
        assert_eq!(engine.learner().typo_count("gan", "guan"), 1);
    }
}

#[test]
fn context_can_favor_literal_reading_without_hiding_corrected_word() {
    for asynchronous in [false, true] {
        let mut engine = engine(asynchronous);
        engine.set_rescoring_context(Some("衣服".into()));
        engine.set_input("meiganxi");
        settle(&mut engine);
        let items = engine.query().unwrap().candidates.items;
        assert_eq!(items[0].text, "没敢洗");
        assert!(items.iter().any(|c| c.text == "没关系"));
        // 应用光标前已是句号：不能沿用上一句的「衣服」。
        engine.set_rescoring_context(Some("衣服。".into()));
        settle(&mut engine);
        assert_eq!(engine.query().unwrap().candidates.items[0].text, "没关系");
    }
}

#[test]
fn recent_commit_supplies_context_but_empty_application_context_overrides_it() {
    let mut engine = engine(false);
    engine.set_input("yifu");
    let clothing = engine.query().unwrap().candidates.items[0].clone();
    engine.commit(&clothing);
    engine.set_input("meiganxi");
    assert_eq!(engine.query().unwrap().candidates.items[0].text, "没敢洗");
    engine.set_rescoring_context(Some(String::new()));
    assert_eq!(engine.query().unwrap().candidates.items[0].text, "没关系");
}

#[test]
fn repeated_explicit_choices_can_overcome_the_correction_prior() {
    let mut engine = engine(false).with_learner(Box::<WordLearner>::default());
    engine.set_input("meiganxi");
    assert_eq!(engine.query().unwrap().candidates.items[0].text, "没关系");
    for _ in 0..10 {
        engine.set_input("meiganxi");
        let literal = engine
            .query()
            .unwrap()
            .candidates
            .items
            .into_iter()
            .find(|c| c.text == "没敢洗")
            .unwrap();
        engine.commit(&literal);
        engine.break_chain();
    }
    engine.set_input("meiganxi");
    assert_eq!(engine.query().unwrap().candidates.items[0].text, "没敢洗");
}

#[test]
fn literal_alternative_is_available_without_a_neural_model() {
    let mut engine = Engine::new(
        Dictionary::parse(
            "没关系\tmei guan xi\t800000\n没\tmei\t900000\n敢\tgan\t200000\n洗\txi\t100000\n",
        )
        .unwrap(),
    )
    .with_language_model(Box::new(Model));
    engine.set_input("meiganxi");
    let items = engine.query().unwrap().candidates.items;
    assert_eq!(items[0].text, "没关系");
    assert!(items.iter().any(|c| c.text == "没敢洗"));
}
