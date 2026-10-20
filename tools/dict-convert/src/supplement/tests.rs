//! 补词产物的一致性、重复执行与坏输入保护。

use super::*;
use qingjian_core::sentence::LanguageModel;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

fn directory() -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!(
        "qingjian-supplement-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn supplements_both_files_without_replacing_counts_and_is_idempotent() {
    let path = directory();
    let dictionary = Dictionary::parse("开源\tkai yuan\t200\n").unwrap();
    let model = BigramModel::parse("<s>\t100\n开源\t200\n", "<s>\t开源\t20\n").unwrap();
    let metadata = Metadata {
        name: "测试".to_owned(),
        license: "MIT".to_owned(),
        ..Metadata::default()
    };
    dictionary
        .write_qj(&path.join("dict.qj"), &metadata)
        .unwrap();
    model.write_qj(&path.join("lm.qj"), &metadata).unwrap();
    std::fs::write(
        path.join("words.tsv"),
        "开源\t999\tkai yuan\n内卷\t100\tnei juan\n",
    )
    .unwrap();
    let run = || {
        convert(
            &path.join("dict.qj"),
            &path.join("lm.qj"),
            &path.join("words.tsv"),
            "GPL-3.0-or-later",
            &path,
        )
        .unwrap()
    };
    run();
    let dictionary = Dictionary::from_path(path.join("dict.qj")).unwrap();
    assert_eq!(dictionary.len(), 2);
    let original = dictionary.entries().find(|hit| hit.text == "开源").unwrap();
    assert_eq!(original.frequency, 200);
    let model = BigramModel::from_path(&path.join("lm.qj")).unwrap();
    assert_eq!(model.word_count(), 3);
    assert_eq!(model.bigram_count(), 1);
    assert!(
        (model.log_prob(None, "开源").unwrap() - (0.8_f64 * 0.2 + 0.2 * 200.0 / 300.0).ln()).abs()
            < 1e-12
    );
    assert!(model.log_prob(None, "内卷").is_some());
    let dict_bytes = std::fs::read(path.join("dict.qj")).unwrap();
    let lm_bytes = std::fs::read(path.join("lm.qj")).unwrap();
    run();
    assert_eq!(dict_bytes, std::fs::read(path.join("dict.qj")).unwrap());
    assert_eq!(lm_bytes, std::fs::read(path.join("lm.qj")).unwrap());
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn invalid_words_leave_existing_outputs_untouched() {
    let path = directory();
    std::fs::write(path.join("dict.qj"), "原文件").unwrap();
    for words in [
        "词库\t0\tci ku\n",
        "词库\t100\tci\n",
        "词库\t100\tci xx\n",
        "",
        "词库\t100\tci ku\n词库\t100\tci ku\n",
    ] {
        std::fs::write(path.join("words.tsv"), words).unwrap();
        assert!(
            convert(
                &path.join("dict.qj"),
                &path.join("lm.qj"),
                &path.join("words.tsv"),
                "GPL-3.0-or-later",
                &path
            )
            .is_err()
        );
        assert_eq!(
            std::fs::read_to_string(path.join("dict.qj")).unwrap(),
            "原文件"
        );
        assert!(!path.join("lm.qj").exists());
    }
    std::fs::remove_dir_all(path).unwrap();
}
