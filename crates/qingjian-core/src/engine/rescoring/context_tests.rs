//! 语境词排序的开关、边界、降级和过期结果回归。

use std::sync::mpsc;
use std::time::{Duration, Instant};

use qingjian_dictionary::Dictionary;

use crate::engine::{Engine, EngineSession};
use crate::sentence::SentenceScorer;

struct Semantic;

impl SentenceScorer for Semantic {
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

fn engine(asynchronous: bool) -> Engine {
    let engine = Engine::new(
        Dictionary::parse("在\tzai\t200\n再\tzai\t100\n再见\tzai jian\t9999\n").unwrap(),
    );
    let mut engine = if asynchronous {
        engine.with_async_sentence_scorer(Box::new(Semantic), None, None, None)
    } else {
        engine.with_sentence_scorer(Box::new(Semantic), None, None, None)
    };
    engine.set_context_ranking(true);
    engine.set_rescoring_context(Some("明天".to_owned()));
    for c in "zai".chars() {
        engine.push(c);
    }
    engine
}

fn first(engine: &Engine) -> String {
    engine.query().unwrap().candidates.items[0].text.clone()
}

#[test]
fn word_context_switches_without_promoting_longer_partial_words() {
    let mut engine = engine(false);
    assert_eq!(first(&engine), "再");
    engine.set_context_ranking(false);
    assert_eq!(first(&engine), "在");
    engine.set_context_ranking(true);
    assert_eq!(first(&engine), "再");
    engine.set_rescoring_context(Some(String::new()));
    engine.history_mut().record("明天");
    assert_eq!(first(&engine), "在", "明确空前文不能退回旧历史");
    engine.set_rescoring_context(None);
    assert_eq!(first(&engine), "再");
}

#[test]
fn recent_history_is_invalidated_on_deletion_focus_and_privacy() {
    for boundary in 0..4 {
        let mut engine = engine(false);
        engine.set_rescoring_context(None);
        engine.history_mut().record("明天");
        assert_eq!(first(&engine), "再");
        match boundary {
            0 => engine.note_backspace(),
            1 => engine.break_chain(),
            2 => engine.set_private(true),
            _ => {
                let mut other = EngineSession::default();
                engine.swap_session(&mut other);
                engine.swap_session(&mut other);
            }
        }
        assert_eq!(first(&engine), "在");
    }
}

#[test]
fn buffer_backspace_preserves_confirmed_history() {
    let mut engine = engine(false);
    engine.set_rescoring_context(None);
    engine.history_mut().record("明天");
    engine.push('a');
    engine.backspace();
    assert_eq!(first(&engine), "再");
}

#[test]
fn async_context_arrives_as_one_complete_result() {
    let mut engine = engine(true);
    assert_eq!(first(&engine), "在");
    assert!(engine.request_rescoring());
    let started = Instant::now();
    while !engine.poll_rescoring() {
        assert!(started.elapsed() < Duration::from_secs(2));
        std::thread::yield_now();
    }
    assert_eq!(first(&engine), "再");
    assert!(!engine.context_ranking.borrow().pending());
    assert_eq!(first(&engine), "再");
}

struct Delayed {
    started: mpsc::SyncSender<()>,

    release: mpsc::Receiver<()>,
}

impl SentenceScorer for Delayed {
    fn score(&self, _: &str, _: &str, texts: &[&str]) -> Vec<f64> {
        vec![0.0; texts.len()]
    }
    fn supports_context(&self) -> bool {
        true
    }
    fn context_gain(&self, _: &str, texts: &[&str]) -> Vec<f64> {
        self.started.send(()).unwrap();
        self.release.recv().unwrap();
        vec![8.0; texts.len()]
    }
}

#[test]
fn a_b_a_and_disabling_discard_late_results() {
    for disable in [false, true] {
        let mut engine = engine(true);
        let (started, ready) = mpsc::sync_channel(1);
        let (release, wait) = mpsc::sync_channel(1);
        engine.set_async_sentence_scorer(Some(Box::new(Delayed {
            started,
            release: wait,
        })));
        first(&engine);
        assert!(engine.request_rescoring());
        ready.recv_timeout(Duration::from_secs(2)).unwrap();
        if disable {
            engine.set_context_ranking(false);
        } else {
            engine.set_rescoring_context(Some("其他".to_owned()));
            engine.set_rescoring_context(Some("明天".to_owned()));
        }
        release.send(()).unwrap();
        std::thread::sleep(Duration::from_millis(20));
        assert!(!engine.poll_rescoring());
        assert_eq!(first(&engine), "在");
    }
}

#[test]
fn invalid_scores_fall_back_and_pool_is_bounded() {
    use super::ContextRanking;
    let mut state = ContextRanking::default();
    state.enabled = true;
    state.prepare("前文".to_owned(), "zai");
    let texts: Vec<String> = (1..=100).map(|n| "字".repeat(n)).collect();
    state.collect(texts.iter().map(String::as_str));
    let pool = state.take();
    assert_eq!(pool.len(), 12);
    state.accept(&pool, &[f64::NAN; 12]);
    assert_eq!(state.bonus("字"), 0.0);
    state.accept(&pool, &[0.0; 11]);
    assert_eq!(state.bonus("字"), 0.0);
}
