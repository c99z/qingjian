//! 完整匹配词参与整句竞争：裁剪、上下文评分、模型重排与用户改选的回归。

use super::*;

// 七个同音整词使低词频的「开源」被六词跨度上限裁掉，词级语言模型仍把它排第一。
const WORDS: &str = "开源\tkai yuan\t1\n开元\tkai yuan\t100\n开园\tkai yuan\t100\n开原\tkai yuan\t100\n开远\tkai yuan\t100\n开缘\tkai yuan\t100\n开猿\tkai yuan\t100\n开\tkai\t10000\n援\tyuan\t10000\n吗\tma\t10000\n";

struct WordModel {
    word_score: f64,
}

impl LanguageModel for WordModel {
    fn log_prob(&self, _: Option<&str>, word: &str) -> Option<f64> {
        Some(match word {
            "开源" => self.word_score,
            "开" | "援" | "吗" => -1.0,
            _ => -10.0,
        })
    }
}

fn competition_engine(word_score: f64) -> Engine {
    Engine::new(Dictionary::parse(WORDS).unwrap())
        .with_language_model(Box::new(WordModel { word_score }))
}

#[test]
fn exact_word_pruned_from_sentence_graph_still_competes_by_score() {
    let mut engine = competition_engine(-0.5);
    engine.set_input("kaiyuan");
    let query = engine.query().unwrap();
    assert_eq!(query.candidates.items[0].text, "开源");
    assert_eq!(query.candidates.items[0].kind, CandidateKind::Chinese);
    assert!(
        query
            .candidates
            .items
            .iter()
            .skip(1)
            .any(|c| c.text == "开援")
    );
}

#[test]
fn more_probable_sentence_can_still_lead() {
    let mut engine = competition_engine(-5.0);
    engine.set_input("kaiyuan");
    let query = engine.query().unwrap();
    assert_eq!(query.candidates.items[0].text, "开援");
    assert_eq!(query.candidates.items[0].kind, CandidateKind::Sentence);
    assert!(query.candidates.items.iter().any(|c| c.text == "开源"));

    engine.set_input("kaiyuanma");
    assert_eq!(engine.query().unwrap().candidates.items[0].text, "开援吗");
}

#[test]
fn selecting_exact_word_helps_it_compete_with_a_similarly_scored_sentence() {
    let mut engine =
        competition_engine(-2.1).with_learner(Box::new(CountingLearner(HashMap::new())));
    engine.set_input("kaiyuan");
    let word = engine
        .query()
        .unwrap()
        .candidates
        .items
        .into_iter()
        .find(|c| c.text == "开源")
        .unwrap();
    engine.commit(&word);
    engine.clear();
    engine.break_chain();
    engine.set_input("kaiyuan");
    assert_eq!(engine.query().unwrap().candidates.items[0].text, "开源");
}

struct WholeWordScorer;

impl SentenceScorer for WholeWordScorer {
    fn score(&self, _: &str, _: &str, texts: &[&str]) -> Vec<f64> {
        texts
            .iter()
            .map(|t| if *t == "开源" { -0.1 } else { -20.0 })
            .collect()
    }
}

#[test]
fn neural_scorer_also_sees_the_recovered_exact_word() {
    let mut engine = competition_engine(-5.0).with_sentence_scorer(
        Box::new(WholeWordScorer),
        Some(1.0),
        None,
        None,
    );
    engine.set_input("kaiyuan");
    assert_eq!(engine.query().unwrap().candidates.items[0].text, "开源");
}

#[test]
fn one_word_selection_does_not_pin_it_above_a_much_better_sentence() {
    let mut engine =
        competition_engine(-5.0).with_learner(Box::new(CountingLearner(HashMap::new())));
    engine.set_input("kaiyuan");
    let word = engine
        .query()
        .unwrap()
        .candidates
        .items
        .into_iter()
        .find(|c| c.text == "开源")
        .unwrap();
    engine.commit(&word);
    engine.clear();
    engine.break_chain();
    engine.set_input("kaiyuan");
    assert_eq!(engine.query().unwrap().candidates.items[0].text, "开援");
}

#[test]
fn async_scoring_recovers_the_word_and_keeps_the_sentence_available() {
    use std::time::{Duration, Instant};

    let mut engine = competition_engine(-5.0).with_async_sentence_scorer(
        Box::new(WholeWordScorer),
        Some(1.0),
        None,
        None,
    );
    engine.set_input("kaiyuan");
    assert_eq!(engine.query().unwrap().candidates.items[0].text, "开援");
    assert!(engine.request_rescoring());
    let started = Instant::now();
    while !engine.poll_rescoring() {
        assert!(started.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(5));
    }
    let query = engine.query().unwrap();
    assert_eq!(query.candidates.items[0].text, "开源");
    assert!(
        query
            .candidates
            .items
            .iter()
            .skip(1)
            .any(|c| c.text == "开援")
    );
}
