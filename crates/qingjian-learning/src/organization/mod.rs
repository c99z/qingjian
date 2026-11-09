//! 从有限日志快照提取重复上屏词；撤销、来源和读音校验均在本机完成。

mod draft;
mod evidence;

pub use draft::OrganizationDraft;
pub use evidence::WordEvidence;

use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use qingjian_core::parser::is_syllable;
use qingjian_dictionary::Dictionary;
use serde_json::Value;

/// 上限只约束此次快照，不轮转或修改原日志。
pub const MAX_LOG_BYTES: u64 = 4 * 1024 * 1024;

pub fn read_evidence(path: &Path, base: &Dictionary) -> std::io::Result<Vec<WordEvidence>> {
    let mut file = std::fs::File::open(path)?;
    let size = file.metadata()?.len();
    let start = size.saturating_sub(MAX_LOG_BYTES);
    file.seek(SeekFrom::Start(start))?;
    let mut bytes = Vec::new();
    file.take(MAX_LOG_BYTES).read_to_end(&mut bytes)?;
    let text = String::from_utf8_lossy(&bytes);
    // 截到行中间时，即使残片碰巧是合法 JSON 也不能作为证据。
    let text = if start > 0 {
        text.split_once('\n').map_or("", |(_, rest)| rest)
    } else {
        &text
    };
    Ok(collect(
        text,
        jiff::Timestamp::now().as_second(),
        Some(base),
    ))
}

/// 首次版本只收 2–8 个汉字，且必须有逐字对齐的完整拼音。
pub fn valid_word(word: &str, pinyin: &str) -> bool {
    let n = word.chars().count();
    let syllables: Vec<_> = pinyin.split_whitespace().collect();
    (2..=8).contains(&n)
        && word.chars().all(|c| ('\u{3400}'..='\u{9fff}').contains(&c))
        && syllables.len() == n
        && syllables.iter().all(|s| is_syllable(s))
}

fn collect(text: &str, now: i64, base: Option<&Dictionary>) -> Vec<WordEvidence> {
    let mut commits = Vec::<Option<(String, String, String)>>::new();
    let mut ids = BTreeMap::new();
    let mut last_id = None;
    for line in text.lines().filter(|l| l.len() <= 65536) {
        let Ok(row) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        match row["event"].as_str() {
            Some("session") => {
                ids.clear();
                last_id = None;
            }
            Some("retract") => {
                if let Some(index) = row["of"].as_u64().and_then(|id| ids.remove(&id)) {
                    commits[index] = None;
                }
            }
            Some("commit") => {
                let Some(id) = row["id"].as_u64() else {
                    continue;
                };
                if last_id.is_some_and(|previous| id <= previous) {
                    ids.clear();
                }
                last_id = Some(id);
                let Some(t) = row["t"].as_str() else { continue };
                let Ok(timestamp) = t.parse::<jiff::Timestamp>() else {
                    continue;
                };
                let age = now - timestamp.as_second();
                if !(0..=14 * 86400).contains(&age)
                    || row["english"].as_bool() != Some(false)
                    || !matches!(row["source"].as_str(), Some("word" | "sentence" | "cloud"))
                    || row["scope"].as_str() != row["keys"].as_str()
                    || row["scope"].as_str().is_none_or(str::is_empty)
                {
                    continue;
                }
                let Some(word) = row["text"].as_str() else {
                    continue;
                };
                let pinyin = row["pinyin"].as_str().unwrap_or("").replace('\'', " ");
                if !valid_word(word, &pinyin) {
                    continue;
                }
                ids.insert(id, commits.len());
                commits.push(Some((word.to_owned(), pinyin, t[..10].to_owned())));
            }
            _ => {}
        }
    }
    let mut counts: BTreeMap<(String, String), (u32, BTreeSet<String>)> = BTreeMap::new();
    for (word, pinyin, day) in commits.into_iter().flatten() {
        let entry = counts.entry((word, pinyin)).or_default();
        entry.0 += 1;
        entry.1.insert(day);
    }
    let existing: BTreeSet<_> = base
        .into_iter()
        .flat_map(Dictionary::entries)
        .map(|h| (h.text, h.pinyin))
        .collect();
    let mut evidence: Vec<_> = counts
        .into_iter()
        .filter(|(_, (count, days))| *count >= 3 && days.len() >= 2)
        .filter(|((word, pinyin), _)| !existing.contains(&(word.as_str(), pinyin.as_str())))
        .map(|((word, pinyin), (count, days))| WordEvidence {
            word,
            pinyin,
            count,
            days: days.len(),
        })
        .collect();
    evidence.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.word.cmp(&b.word)));
    evidence.truncate(60);
    evidence
}

#[cfg(test)]
mod tests;
