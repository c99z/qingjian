//! 最近选择的落盘、撤销、隐私与中文候选排序回归。

mod english;
mod sentence;

use qingjian_core::sentence::LanguageModel;
use qingjian_core::{Engine, Learner};
use qingjian_dictionary::{Dictionary, WordList};

use super::FrequencyLearner;

const WORDS: &str = "再\tzai\t1000\n在\tzai\t1000\n我\two\t1000\n什么\tshen me\t1000\n";

struct ContextModel;

impl LanguageModel for ContextModel {
    fn log_prob(&self, previous: Option<&str>, word: &str) -> Option<f64> {
        Some(match (previous, word) {
            (Some("我"), "在") | (None, "我") => -1.0,
            (Some("我"), "再") => -7.0,
            (_, "再") => -2.0,
            (_, "在") => -3.0,
            _ => -10.0,
        })
    }
}

fn engine() -> Engine {
    let mut learner = FrequencyLearner::parse("再\t201\n在\t192\n");
    learner.load_choices("zai\t再\t194\nzai\t在\t182\n");
    Engine::new(Dictionary::parse(WORDS).unwrap())
        .with_language_model(Box::new(ContextModel))
        .with_learner(Box::new(learner))
}

fn pick(engine: &mut Engine, input: &str, text: &str) {
    engine.set_input(input);
    let candidate = engine
        .query()
        .unwrap()
        .candidates
        .items
        .into_iter()
        .find(|candidate| candidate.text == text)
        .unwrap();
    engine.commit(&candidate);
}

#[test]
fn context_and_a_recent_choice_can_overcome_old_counts() {
    let mut engine = engine();
    engine.set_input("zai");
    assert_eq!(engine.query().unwrap().candidates.items[0].text, "再");
    pick(&mut engine, "wo", "我");
    engine.set_input("zai");
    assert_eq!(engine.query().unwrap().candidates.items[0].text, "在");
    engine.clear();
    engine.break_chain();
    pick(&mut engine, "zai", "在");
    engine.break_chain();
    engine.set_input("zai");
    assert_eq!(engine.query().unwrap().candidates.items[0].text, "在");
    assert!(
        engine.learner().choice_weight("zai", "再") > engine.learner().choice_weight("zai", "在")
    );
}

#[test]
fn recent_choices_round_trip_and_legacy_rows_have_no_invented_recency() {
    let dir = std::env::temp_dir().join(format!("qingjian-recent-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("user.tsv");
    let mut learner = FrequencyLearner::default();
    learner.load_choices("zai\t再\t194\nzai\t在\t182\n");
    assert_eq!(learner.recent_choice_weight("zai", "再"), 0.0);
    learner.record_choice("zai", "在");
    learner.record_choice("zai", "再");
    let expected = learner.recent_choice_weight("zai", "在");
    learner
        .save_choices_to(&FrequencyLearner::choices_path(&path))
        .unwrap();
    let reloaded = FrequencyLearner::from_path(&path).unwrap();
    assert_eq!(reloaded.choice_weight("zai", "在"), 183);
    assert_eq!(reloaded.recent_choice_weight("zai", "在"), expected);
    assert!(reloaded.recent_choice_weight("zai", "再") > expected);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn undo_compacts_recent_history_and_other_inputs_do_not_age_it() {
    let mut learner = FrequencyLearner::default();
    learner.record_choice("zai", "在");
    learner.record_choice("zai", "再");
    learner.record_choice("wo", "我");
    let before = learner.recent_choice_weight("zai", "在");
    learner.record_choice("zai", "载");
    learner.unrecord_choice("zai", "载");
    assert_eq!(learner.recent_choice_weight("zai", "在"), before);
    learner.unrecord_choice("zai", "在");
    assert_eq!(learner.recent_choice_weight("zai", "再"), 1.0);
    for _ in 0..16 {
        learner.record_choice("zai", "在");
    }
    assert_eq!(learner.recent_choice_weight("zai", "再"), 0.0);
    learner.forget("在");
    assert_eq!(learner.recent_choice_weight("zai", "在"), 0.0);
}

#[test]
fn privacy_and_disabled_learning_preserve_recent_preferences() {
    let mut engine = engine();
    pick(&mut engine, "zai", "在");
    let before = engine.learner().recent_choice_weight("zai", "在");
    engine.set_private(true);
    pick(&mut engine, "zai", "再");
    assert_eq!(engine.learner().recent_choice_weight("zai", "在"), before);
    engine.set_private(false);
    engine.set_learning(false);
    pick(&mut engine, "zai", "再");
    assert_eq!(engine.learner().recent_choice_weight("zai", "在"), before);
}

#[test]
fn learned_pinyin_is_hidden_in_chinese_but_explicit_english_choices_remain() {
    let mut learner = FrequencyLearner::default();
    learner.learn_english("zai");
    learner.learn_english("houxuanshengcheng");
    learner.learn_english("kubectl");
    let mut engine = Engine::new(Dictionary::parse(WORDS).unwrap()).with_learner(Box::new(learner));
    engine.set_input("zai");
    assert!(
        !engine
            .query()
            .unwrap()
            .candidates
            .items
            .iter()
            .any(|c| c.text == "zai")
    );
    engine.set_input("houx");
    assert!(
        !engine
            .query()
            .unwrap()
            .candidates
            .items
            .iter()
            .any(|c| c.text == "houxuanshengcheng")
    );
    engine.set_input("kubectl");
    assert!(
        engine
            .query()
            .unwrap()
            .candidates
            .items
            .iter()
            .any(|c| c.text == "kubectl")
    );
    engine.set_english_mode(true);
    pick(&mut engine, "zai", "zai");
    engine.set_english_mode(false);
    engine.set_input("zai");
    assert!(
        engine
            .query()
            .unwrap()
            .candidates
            .items
            .iter()
            .any(|c| c.text == "zai")
    );
}

#[test]
fn english_global_counts_do_not_override_input_specific_chinese_choices() {
    let mut learner = FrequencyLearner::parse("Sm\t25\n什么\t109\n");
    learner.load_choices("sm\t什么\t10\n");
    let mut engine = Engine::new(Dictionary::parse(WORDS).unwrap())
        .with_learner(Box::new(learner))
        .with_english(WordList::parse("Sm\tsm\t3710\ndata\tdata\t5000\n").unwrap());
    engine.set_input("sm");
    assert_eq!(engine.query().unwrap().candidates.items[0].text, "什么");
    engine.set_input("data");
    assert!(
        engine
            .query()
            .unwrap()
            .candidates
            .items
            .iter()
            .any(|c| c.text == "data")
    );
}
