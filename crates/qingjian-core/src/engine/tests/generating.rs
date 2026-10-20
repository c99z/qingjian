//! 已能整词匹配的简拼不能触发生成兜底，后台结果也不能把末字误改成英文字母。

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use qingjian_dictionary::Dictionary;

use crate::candidate::CandidateKind;
use crate::engine::Engine;
use crate::sentence::SentenceScorer;

struct Generates(Arc<AtomicUsize>);

impl SentenceScorer for Generates {
    fn score(&self, _: &str, _: &str, texts: &[&str]) -> Vec<f64> {
        vec![-1.0; texts.len()]
    }

    fn generate(&self, keys: &str, _: usize, _: usize) -> Vec<String> {
        self.0.fetch_add(1, Ordering::SeqCst);
        vec![if keys == "yongdockerbushuhenfangbian" {
            "用docker部署很方便".into()
        } else {
            "候选C".into()
        }]
    }
}

fn engine(whole_word: bool, asynchronous: bool) -> (Engine, Arc<AtomicUsize>) {
    let mut dictionary = String::from("候选\thou xuan\t1000\n后\thou\t100\n用\tyong\t100\n");
    if whole_word {
        dictionary.push_str("候选词\thou xuan ci\t1000\n");
    }
    let calls = Arc::new(AtomicUsize::new(0));
    let engine = Engine::new(Dictionary::parse(&dictionary).unwrap());
    let scorer = Box::new(Generates(Arc::clone(&calls)));
    let engine = if asynchronous {
        engine.with_async_sentence_scorer(scorer, None, None, None)
    } else {
        engine.with_sentence_scorer(scorer, None, None, None)
    };
    (engine, calls)
}

fn settle(engine: &mut Engine) {
    let started = Instant::now();
    loop {
        engine.query().unwrap();
        if !engine.request_rescoring() {
            break;
        }
        while !engine.poll_rescoring() {
            assert!(started.elapsed() < Duration::from_secs(3));
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

#[test]
fn abbreviated_word_does_not_become_a_generated_latin_suffix() {
    for asynchronous in [false, true] {
        let (mut engine, calls) = engine(true, asynchronous);
        engine.set_input("houxuanc");
        assert_eq!(engine.query().unwrap().candidates.items[0].text, "候选词");
        settle(&mut engine);
        let items = engine.query().unwrap().candidates.items;
        assert_eq!(items[0].text, "候选词");
        assert!(!items.iter().any(|c| c.kind == CandidateKind::Generated));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }
}

#[test]
fn a_prefix_word_does_not_block_generation_for_the_remaining_input() {
    let (mut engine, calls) = engine(false, false);
    engine.set_input("houxuanc");
    let first = &engine.query().unwrap().candidates.items[0];
    assert_eq!(first.text, "候选C");
    assert_eq!(first.kind, CandidateKind::Generated);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn generation_still_handles_english_inside_a_sentence() {
    let (mut engine, calls) = engine(true, true);
    engine.set_input("yongdockerbushuhenfangbian");
    settle(&mut engine);
    let first = &engine.query().unwrap().candidates.items[0];
    assert_eq!(first.text, "用docker部署很方便");
    assert_eq!(first.kind, CandidateKind::Generated);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn an_old_generated_result_is_ignored_after_a_matching_word_is_added() {
    let (mut engine, calls) = engine(false, false);
    engine.set_input("houxuanc");
    assert_eq!(engine.query().unwrap().candidates.items[0].text, "候选C");
    engine.set_extra_dictionaries(vec![
        Dictionary::parse("候选词\thou xuan ci\t1000\n").unwrap(),
    ]);
    assert_eq!(engine.query().unwrap().candidates.items[0].text, "候选词");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}
