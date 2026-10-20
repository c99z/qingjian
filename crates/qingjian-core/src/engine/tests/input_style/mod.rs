//! 中文策略对英文词、拼接和主动英文输入的边界。

mod generating;
mod prediction;

use qingjian_dictionary::{Dictionary, WordList};

use crate::{CandidateKind, ChineseInputStyle, CustomPhrase, Engine};

fn engine() -> Engine {
    Engine::new(
        Dictionary::parse("什么\tshen me\t1000\n开发\tkai fa\t1000\n候选词\thou xuan ci\t1000\n")
            .unwrap(),
    )
    .with_english(
        WordList::parse("Sm\tsm\t5000\nrust\trust\t4000\nimport\timport\t4000\nvery\tvery\t4000\n")
            .unwrap(),
    )
}

#[test]
fn focus_hides_english_words_and_completion_but_switching_restores_them() {
    let mut engine = engine();
    for input in ["sm", "rust", "impor", "very"] {
        engine.set_chinese_input_style(ChineseInputStyle::Mixed);
        engine.set_input(input);
        assert!(
            engine
                .query()
                .unwrap()
                .candidates
                .items
                .iter()
                .any(|c| c.kind == CandidateKind::English)
        );
        engine.set_chinese_input_style(ChineseInputStyle::Chinese);
        // 不能切拼音的输入允许没有候选，回车仍可原样上屏。
        if let Ok(query) = engine.query() {
            assert!(
                query
                    .candidates
                    .items
                    .iter()
                    .all(|c| c.kind != CandidateKind::English)
            );
        }
        assert_eq!(engine.take_raw(), input);
    }
    engine.set_input("sm");
    assert_eq!(engine.query().unwrap().candidates.items[0].text, "什么");
    engine.set_english_mode(true);
    assert!(
        engine
            .query()
            .unwrap()
            .candidates
            .items
            .iter()
            .any(|c| c.text == "Sm")
    );
}

#[test]
fn focus_disables_automatic_english_tail_without_changing_typed_keys() {
    let mut engine = engine();
    engine.set_input("kaifarust");
    assert_eq!(engine.query().unwrap().candidates.items[0].text, "开发rust");
    engine.set_chinese_input_style(ChineseInputStyle::Chinese);
    assert!(
        engine
            .query()
            .unwrap()
            .candidates
            .items
            .iter()
            .all(|c| c.text != "开发rust")
    );
    assert_eq!(engine.composition().text(), "kaifarust");
    engine.set_chinese_input_style(ChineseInputStyle::Mixed);
    assert_eq!(engine.query().unwrap().candidates.items[0].text, "开发rust");
}

#[test]
fn explicit_custom_phrases_and_raw_input_remain_available() {
    let mut engine = engine();
    engine.set_chinese_input_style(ChineseInputStyle::Chinese);
    engine
        .set_custom_phrases(vec![CustomPhrase {
            code: "mail".into(),
            text: "hello@example.com".into(),
            position: 1,
            enabled: true,
        }])
        .unwrap();
    engine.set_input("mail");
    assert_eq!(
        engine.query().unwrap().candidates.items[0].text,
        "hello@example.com"
    );
    engine.set_input("houxuanc");
    assert_eq!(engine.query().unwrap().candidates.items[0].text, "候选词");
    assert_eq!(engine.take_raw(), "houxuanc");
}

#[test]
fn focus_does_not_learn_raw_pinyin_as_english_but_english_mode_does() {
    use crate::{Candidate, Learner};
    use std::sync::{Arc, Mutex};

    struct Recorder(Arc<Mutex<Vec<String>>>);
    impl Learner for Recorder {
        fn record(&mut self, _: &Candidate) {}
        fn weight(&self, _: &str) -> u32 {
            0
        }
        fn learn_english(&mut self, word: &str) {
            self.0.lock().unwrap().push(word.to_owned());
        }
    }
    let learned = Arc::new(Mutex::new(Vec::new()));
    let mut engine = engine().with_learner(Box::new(Recorder(learned.clone())));
    engine.set_chinese_input_style(ChineseInputStyle::Chinese);
    engine.set_input("kubectl");
    engine.take_raw();
    assert!(learned.lock().unwrap().is_empty());
    engine.set_english_mode(true);
    engine.set_input("kubectl");
    engine.take_raw();
    assert_eq!(*learned.lock().unwrap(), ["kubectl"]);
}

#[test]
fn switching_style_rechecks_correction_previously_blocked_by_an_english_word() {
    let mut engine = Engine::new(Dictionary::parse("和\the\t50000\n咯\tlo\t100\n").unwrap())
        .with_english(WordList::parse("hello\n").unwrap());
    engine.set_input("hello");
    assert!(engine.query().unwrap().correction.is_none());
    engine.set_chinese_input_style(ChineseInputStyle::Chinese);
    let query = engine.query().unwrap();
    assert!(query.correction.is_some());
    assert_eq!(query.candidates.items[0].text, "和咯");
    engine.set_chinese_input_style(ChineseInputStyle::Mixed);
    assert!(engine.query().unwrap().correction.is_none());
}
