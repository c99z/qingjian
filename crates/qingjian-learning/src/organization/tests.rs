//! 整理只使用可确认、未撤销且跨日重复的证据；不从原样拼音或不完整日志造词。

use super::{OrganizationDraft, collect, valid_word};
use qingjian_dictionary::Dictionary;
use serde_json::json;

fn row(id: u64, day: u8, text: &str) -> serde_json::Value {
    json!({"event":"commit","id":id,"t":format!("2026-11-{day:02}T03:00:00+08:00"),"text":text,"scope":"lubao","keys":"lubao","pinyin":"lu'bao","english":false,"source":"word","app":"private-project"})
}

fn now() -> i64 {
    "2026-11-06T06:00:00+08:00"
        .parse::<jiff::Timestamp>()
        .unwrap()
        .as_second()
}

#[test]
fn repeated_cross_day_commits_exclude_retracted_and_incomplete() {
    let mut rows = vec![
        row(1, 4, "录包"),
        row(2, 4, "录包"),
        row(3, 5, "录包"),
        row(4, 5, "陆爆"),
    ];
    rows.push(json!({"event":"retract","of":4,"text":"陆爆","chosen":"录包"}));
    let mut partial = row(5, 5, "陆爆");
    partial["keys"] = json!("lu");
    rows.push(partial);
    let evidence = collect(
        &rows
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n"),
        now(),
        None,
    );
    assert_eq!(evidence.len(), 1);
    assert_eq!(evidence[0].word, "录包");
    assert_eq!(evidence[0].count, 3);
    assert_eq!(evidence[0].days, 2);
    let sent = serde_json::to_string(&evidence).unwrap();
    assert!(!sent.contains("private-project"));
    assert!(!sent.contains("lubao"));
    let base = Dictionary::parse("路霸\tlu bao\t1000\n").unwrap();
    let draft = OrganizationDraft::build(&evidence, &[0], &base).unwrap();
    assert!(draft.tsv.contains("录包\tlu bao\t3"));
    assert!(OrganizationDraft::build(&evidence, &[1], &base).is_err());
    assert!(OrganizationDraft::build(&evidence, &[0, 0], &base).is_err());
    let base = Dictionary::parse("录包\tlu bao\t999\n").unwrap();
    assert_eq!(
        OrganizationDraft::build(&evidence, &[0], &base)
            .unwrap()
            .words,
        0
    );
}

#[test]
fn raw_stale_same_day_and_misaligned_text_never_become_evidence() {
    let mut rows = Vec::new();
    for i in 1..=5 {
        rows.push(row(i, 4, "录包"));
        let mut raw = row(i + 10, 5, "拼音");
        raw["source"] = json!("raw");
        rows.push(raw);
        rows.push(row(i + 20, 5, "录包词"));
        let mut old = row(i + 30, 1, "陆爆");
        old["t"] = json!("2026-10-01T01:00:00Z");
        rows.push(old);
    }
    assert!(
        collect(
            &rows
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n"),
            now(),
            None
        )
        .is_empty()
    );
    assert!(!valid_word("sm", "s m"));
    assert!(!valid_word("录包", "lu invalid"));
}

#[test]
fn session_ids_are_not_retracted_across_restarts() {
    let rows = [
        row(1, 4, "录包"),
        row(2, 4, "录包"),
        json!({"event":"session"}),
        row(1, 5, "录包"),
        row(2, 5, "陆爆"),
        json!({"event":"retract","of":2}),
    ];
    let evidence = collect(
        &rows
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n"),
        now(),
        None,
    );
    assert_eq!(evidence[0].count, 3);
}

#[test]
fn existing_words_cannot_exhaust_the_budget_for_new_personal_terms() {
    let mut rows = Vec::new();
    let mut tsv = String::new();
    let mut id = 1;
    for suffix in '\u{4e00}'..='\u{4e40}' {
        let word = format!("词{suffix}");
        tsv.push_str(&format!("{word}\tlu bao\t100\n"));
        for day in [4, 4, 5, 5] {
            rows.push(row(id, day, &word));
            id += 1;
        }
    }
    for day in [4, 4, 5] {
        rows.push(row(id, day, "录包"));
        id += 1;
    }
    let text = rows
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(!collect(&text, now(), None).iter().any(|e| e.word == "录包"));
    let base = Dictionary::parse(&tsv).unwrap();
    let evidence = collect(&text, now(), Some(&base));
    assert_eq!(evidence.len(), 1);
    assert_eq!(evidence[0].word, "录包");
}
