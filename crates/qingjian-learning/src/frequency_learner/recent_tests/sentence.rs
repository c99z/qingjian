//! 用户整词与模型拆字路径竞争时，同码累计和最近选择也必须参与。

use std::time::{Duration, Instant};

use qingjian_core::sentence::{LanguageModel, SentenceScorer};
use qingjian_core::{Engine, Learner};
use qingjian_dictionary::Dictionary;

use super::super::FrequencyLearner;
use super::pick;

struct SplitModel;

impl LanguageModel for SplitModel {
    fn log_prob(&self, _: Option<&str>, word: &str) -> Option<f64> {
        Some(match word {
            "录包" => -3.0,
            "陆" | "爆" => -1.0,
            _ => -20.0,
        })
    }
}

impl SentenceScorer for SplitModel {
    fn score(&self, _: &str, _: &str, texts: &[&str]) -> Vec<f64> {
        texts
            .iter()
            .map(|text| if *text == "录包" { -6.0 } else { -2.0 })
            .collect()
    }
}

fn engine(pruned: bool, asynchronous: bool) -> Engine {
    let mut words = String::from("陆\tlu\t10000\n爆\tbao\t10000\n");
    if pruned {
        for text in ["陆宝", "陆保", "鹿宝", "路宝", "鲁宝", "璐宝"] {
            words.push_str(&format!("{text}\tlu bao\t1000\n"));
        }
    }
    let mut learner = FrequencyLearner::default();
    learner.learn_word("录包", &["lu".into(), "bao".into()]);
    let engine = Engine::new(Dictionary::parse(&words).unwrap())
        .with_learner(Box::new(learner))
        .with_language_model(Box::new(SplitModel));
    if asynchronous {
        engine.with_async_sentence_scorer(Box::new(SplitModel), Some(0.5), None, None)
    } else {
        engine.with_sentence_scorer(Box::new(SplitModel), Some(0.5), None, None)
    }
}

fn settle(engine: &mut Engine) {
    let started = Instant::now();
    loop {
        engine.query().unwrap();
        if !engine.request_rescoring() {
            break;
        }
        while !engine.poll_rescoring() {
            assert!(started.elapsed() < Duration::from_secs(5));
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

#[test]
fn recent_word_choice_survives_neural_reranking_and_graph_pruning() {
    for pruned in [false, true] {
        for asynchronous in [false, true] {
            let mut engine = engine(pruned, asynchronous);
            engine.set_input("lubao");
            settle(&mut engine);
            assert_eq!(engine.query().unwrap().candidates.items[0].text, "陆爆");
            pick(&mut engine, "lubao", "录包");
            engine.break_chain();
            engine.set_input("lu'bao");
            settle(&mut engine);
            assert_eq!(engine.learner().recent_choice_weight("lubao", "录包"), 1.0);
            assert_eq!(
                engine.query().unwrap().candidates.items[0].text,
                "录包",
                "pruned={pruned}, asynchronous={asynchronous}"
            );
            assert!(
                engine
                    .query()
                    .unwrap()
                    .candidates
                    .items
                    .iter()
                    .skip(1)
                    .any(|c| c.text == "陆爆")
            );
        }
    }
}

#[test]
fn accumulated_choices_also_participate_without_a_neural_model() {
    let mut learner = FrequencyLearner::default();
    learner.learn_word("录包", &["lu".into(), "bao".into()]);
    learner.load_choices("lubao\t录包\t10\n");
    let mut engine = Engine::new(Dictionary::parse("陆\tlu\t10000\n爆\tbao\t10000\n").unwrap())
        .with_learner(Box::new(learner))
        .with_language_model(Box::new(SplitModel));
    assert_eq!(engine.learner().recent_choice_weight("lubao", "录包"), 0.0);
    engine.set_input("lubao");
    assert_eq!(engine.query().unwrap().candidates.items[0].text, "录包");
}
