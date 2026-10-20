//! 纠错总开关覆盖整段修正、词图与迟到生成；模糊音和英文输入保持独立。

use qingjian_dictionary::{Dictionary, WordList};

use crate::{Engine, FuzzyRules};

#[test]
fn switching_off_removes_typo_paths_and_clears_cached_whole_input_correction() {
    let mut engine = super::engine(false);
    engine.set_input("meiganxi");
    assert_eq!(engine.query().unwrap().candidates.items[0].text, "没关系");
    engine.set_pinyin_correction(false);
    assert!(
        engine
            .query()
            .unwrap()
            .candidates
            .items
            .iter()
            .all(|c| c.text != "没关系")
    );
    engine.set_pinyin_correction(true);
    assert_eq!(engine.query().unwrap().candidates.items[0].text, "没关系");

    let mut engine = Engine::new(Dictionary::parse("明天\tming tian\t1000\n").unwrap());
    engine.set_input("mingtain");
    assert!(engine.query().unwrap().correction.is_some());
    engine.set_pinyin_correction(false);
    assert!(engine.query().unwrap().correction.is_none());
    engine.set_pinyin_correction(true);
    assert!(engine.query().unwrap().correction.is_some());
}

#[test]
fn explicit_fuzzy_rules_and_english_spelling_suggestions_are_independent() {
    let mut engine = Engine::new(Dictionary::parse("知识\tzhi shi\t1000\n").unwrap())
        .with_english(WordList::parse("receive\n").unwrap());
    engine.set_pinyin_correction(false);
    engine.set_fuzzy(FuzzyRules {
        z_zh: true,
        ..Default::default()
    });
    engine.set_input("zishi");
    assert_eq!(engine.query().unwrap().candidates.items[0].text, "知识");
    engine.set_english_mode(true);
    engine.set_input("recieve");
    assert!(
        engine
            .query()
            .unwrap()
            .candidates
            .items
            .iter()
            .any(|c| c.text == "receive")
    );
}

#[test]
fn a_late_generated_correction_is_not_used_after_switching_off() {
    use crate::CandidateKind;
    use crate::sentence::SentenceScorer;

    struct Generator;
    impl SentenceScorer for Generator {
        fn score(&self, _: &str, _: &str, texts: &[&str]) -> Vec<f64> {
            vec![-1.0; texts.len()]
        }
        fn generate(&self, _: &str, _: usize, _: usize) -> Vec<String> {
            vec!["候选词".into()]
        }
    }
    let mut engine = Engine::new(Dictionary::parse("候选\thou xuan\t1000\n").unwrap())
        .with_async_sentence_scorer(Box::new(Generator), None, None, None);
    engine.set_input("houxuanc");
    engine.query().unwrap();
    assert!(engine.request_rescoring());
    engine.set_pinyin_correction(false);
    let start = std::time::Instant::now();
    while !engine.poll_rescoring() {
        assert!(start.elapsed() < std::time::Duration::from_secs(3));
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    super::settle(&mut engine);
    assert!(
        engine
            .query()
            .unwrap()
            .candidates
            .items
            .iter()
            .all(|c| c.kind != CandidateKind::Generated)
    );
    engine.set_pinyin_correction(true);
    super::settle(&mut engine);
    assert_eq!(engine.query().unwrap().candidates.items[0].text, "候选词");
}

#[test]
fn cloud_words_no_longer_tolerate_spelling_errors_when_disabled() {
    use crate::Prediction;
    use crate::engine::tests::{EchoPredictor, cloud};
    for enabled in [true, false] {
        let mut engine = super::engine(false).with_predictor(Box::new(EchoPredictor {
            submitted: Default::default(),
            replies: vec![Prediction {
                sequence: 1,
                words: vec![cloud("开发", &["kai", "fa"])],
                sentence: None,
            }],
            sentence: false,
        }));
        engine.set_pinyin_correction(enabled);
        engine.set_input("kaia");
        engine.request_prediction(None, &[]);
        assert_eq!(
            engine.poll_prediction().unwrap().words.len(),
            usize::from(enabled)
        );
    }
}
