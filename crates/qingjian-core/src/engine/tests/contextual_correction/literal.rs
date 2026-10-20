//! 高频纠错变体不能在格子裁剪时挤掉原拼音，原读音须独立参与最终比较。

use qingjian_dictionary::Dictionary;

use crate::Engine;
use crate::sentence::LanguageModel;

struct Model;

impl LanguageModel for Model {
    fn log_prob(&self, _: Option<&str>, word: &str) -> Option<f64> {
        Some(match word {
            "指的" | "是" => -1.0,
            "时" => -2.0,
            _ => -3.0,
        })
    }
}

#[test]
fn frequent_typo_words_cannot_crowd_out_the_literal_sentence() {
    // 六个高频 shi de 词填满同一跨度，让低词频但原拼音正确的「指的」被预选裁掉。
    let words = "指的\tzhi de\t1\n是的\tshi de\t1000000\n使得\tshi de\t1000000\n实得\tshi de\t1000000\n识得\tshi de\t1000000\n失德\tshi de\t1000000\n诗的\tshi de\t1000000\n是\tshi\t10000\n时\tshi\t9000\n";
    for asynchronous in [false, true] {
        let mut engine =
            Engine::new(Dictionary::parse(words).unwrap()).with_language_model(Box::new(Model));
        if asynchronous {
            engine =
                engine.with_async_sentence_scorer(Box::new(super::LiteralScorer), None, None, None);
        }
        engine.set_input("zhideshi");
        assert_eq!(engine.query().unwrap().candidates.items[0].text, "指的是");
        super::settle(&mut engine);
        assert_eq!(engine.query().unwrap().candidates.items[0].text, "指的是");
        engine.set_pinyin_correction(false);
        super::settle(&mut engine);
        assert_eq!(engine.query().unwrap().candidates.items[0].text, "指的是");
    }
}
