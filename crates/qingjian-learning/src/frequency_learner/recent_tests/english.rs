//! 英文选择的撤销与删除，以及误学拼音对混输的影响。

use qingjian_core::{Engine, Learner};
use qingjian_dictionary::{Dictionary, WordList};

use super::{WORDS, engine, pick};
use crate::frequency_learner::FrequencyLearner;

#[test]
fn replacing_an_english_commit_retracts_its_recent_preference() {
    let mut engine = engine().with_english(WordList::parse("Sm\tsm\t3710\n").unwrap());
    pick(&mut engine, "sm", "Sm");
    assert_eq!(engine.learner().choice_weight("sm", "Sm"), 1);
    engine.note_backspace();
    engine.note_backspace();
    pick(&mut engine, "sm", "什么");
    assert_eq!(engine.learner().choice_weight("sm", "Sm"), 0);
    assert_eq!(engine.learner().recent_choice_weight("sm", "Sm"), 0.0);
}

#[test]
fn forgetting_english_also_removes_its_explicit_choice_evidence() {
    let mut learner = FrequencyLearner::default();
    learner.learn_english("Zai");
    learner.record_choice("zai", "Zai");
    assert!(learner.forget_english("zAI"));
    assert_eq!(learner.choice_weight("zai", "Zai"), 0);
    assert_eq!(learner.recent_choice_weight("zai", "Zai"), 0.0);
}

#[test]
fn old_learned_pinyin_cannot_become_an_english_tail() {
    let mut learner = FrequencyLearner::default();
    learner.learn_english("houxuanshengcheng");
    let mut engine = Engine::new(Dictionary::parse(WORDS).unwrap()).with_learner(Box::new(learner));
    engine.set_input("wohouxuanshengcheng");
    assert!(
        engine
            .query()
            .unwrap()
            .candidates
            .items
            .iter()
            .all(|c| !c.text.contains("houxuanshengcheng"))
    );
}
