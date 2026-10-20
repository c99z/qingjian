//! 专注中文过滤组句云联想，主动翻译仍可输出英文。

use crate::engine::tests::{EchoPredictor, cloud, engine};
use crate::{ChineseInputStyle, Prediction};

#[test]
fn delayed_cloud_completion_obeys_focus_but_explicit_translation_does_not() {
    for translate in [false, true] {
        let mut engine = engine().with_predictor(Box::new(EchoPredictor {
            submitted: Default::default(),
            replies: vec![Prediction {
                sequence: 1,
                words: vec![cloud("开发", &["kai", "fa"]), cloud("开F", &["kai", "fa"])],
                sentence: Some("开发Rust".into()),
            }],
            sentence: true,
        }));
        engine.set_input("kaifa");
        if translate {
            assert_eq!(engine.request_translation("开发"), Some(1));
        } else {
            assert_eq!(engine.request_prediction(None, &[]), Some(1));
        }
        engine.set_chinese_input_style(ChineseInputStyle::Chinese);
        let reply = engine.poll_prediction().unwrap();
        if translate {
            assert_eq!(reply.sentence.as_deref(), Some("开发Rust"));
        } else {
            assert!(reply.sentence.is_none());
            assert_eq!(reply.words.len(), 1);
            assert_eq!(reply.words[0].text, "开发");
        }
    }
}
