//! 整句的语境增益不替换基础分，空语境仍保持原排序。

use qingjian_dictionary::Dictionary;

use crate::Engine;
use crate::sentence::{Conversion, SentenceScorer, SentenceWord};

struct SemanticPath;

impl SentenceScorer for SemanticPath {
    fn score(&self, _: &str, _: &str, texts: &[&str]) -> Vec<f64> {
        vec![-1.0; texts.len()]
    }

    fn supports_context(&self) -> bool {
        true
    }

    fn context_gain(&self, _: &str, texts: &[&str]) -> Vec<f64> {
        texts
            .iter()
            .map(|text| if text.ends_with("再") { 8.0 } else { -8.0 })
            .collect()
    }
}

fn path(word: &str, score: f64) -> Conversion {
    Conversion {
        text: format!("明天{word}"),
        syllables: vec!["ming".into(), "tian".into(), "zai".into()],
        words: vec![
            SentenceWord {
                text: "明天".into(),
                syllables: vec!["ming".into(), "tian".into()],
                placeholder: false,
            },
            SentenceWord {
                text: word.into(),
                syllables: vec!["zai".into()],
                placeholder: false,
            },
        ],
        score,
        static_score: score,
        penalty: 0.0,
    }
}

#[test]
fn semantic_path_bonus_is_separate_and_empty_context_restores_base() {
    let mut engine = Engine::new(Dictionary::parse("在\tzai\t100\n").unwrap())
        .with_sentence_scorer(Box::new(SemanticPath), None, None, None);
    engine.set_context_ranking(true);
    engine.set_input("mingtianzai");
    engine.set_rescoring_context(Some("如果今天来不及就".into()));
    engine.prepare_context_ranking();
    let mut paths = vec![path("在", -2.0), path("再", -2.2)];
    engine.rescore_paths(&mut paths, "mingtianzai");
    assert_eq!(paths[0].text, "明天在");
    assert!(engine.finish_sync_context());
    engine.rescore_paths(&mut paths, "mingtianzai");
    assert_eq!(paths[0].text, "明天再");
    assert_eq!(paths[0].score, -2.2);
    assert_eq!(paths[1].score, -2.0);
    engine.set_rescoring_context(Some(String::new()));
    engine.prepare_context_ranking();
    engine.rescore_paths(&mut paths, "mingtianzai");
    assert_eq!(paths[0].text, "明天在");
}
