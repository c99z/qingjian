//! 从现成打包数据补人工词条与缺失的一元先验，保留已有语料计数。

use std::collections::{BTreeMap, HashSet};
use std::fmt::Write;
use std::path::Path;

use qingjian_core::parser::is_syllable;
use qingjian_dictionary::{Dictionary, canonical_syllable};
use qingjian_format::Metadata;
use qingjian_lm::BigramModel;

use crate::error::ConvertError;

#[cfg(test)]
mod tests;

pub fn convert(
    dict: &Path,
    lm: &Path,
    words: &Path,
    license: &str,
    out_dir: &Path,
) -> Result<(), ConvertError> {
    let additions = read_words(words)?;
    let dictionary = Dictionary::from_path(dict)?;
    let mut model = BigramModel::from_path(lm)?;
    let mut entries: BTreeMap<(String, String), u32> = BTreeMap::new();
    for hit in dictionary.entries() {
        entries
            .entry((hit.text.to_owned(), hit.pinyin.to_owned()))
            .and_modify(|n| *n = (*n).max(hit.frequency))
            .or_insert(hit.frequency);
    }
    let before = entries.len();
    for ((word, pinyin), count) in &additions {
        entries
            .entry((word.clone(), pinyin.clone()))
            .or_insert(*count);
    }
    let priors: Vec<_> = additions
        .iter()
        .map(|((word, _), count)| (word.as_str(), *count))
        .collect();
    let added_priors = model.add_unigram_priors(&priors)?;
    let added_words = entries.len() - before;
    let mut tsv = String::new();
    for ((word, pinyin), count) in &entries {
        writeln!(tsv, "{word}\t{pinyin}\t{count}").unwrap();
    }
    let updated = Dictionary::parse(&tsv)?;
    let dict_meta = supplemented_metadata(dictionary.metadata(), words, license, added_words);
    let lm_meta = supplemented_metadata(model.metadata(), words, license, added_priors);
    // 所有输入和产物先在内存中校验；Writer 原子替换，不会截断仍在 mmap 的源文件。
    updated.write_qj(&out_dir.join("dict.qj"), &dict_meta)?;
    model.write_qj(&out_dir.join("lm.qj"), &lm_meta)?;
    std::fs::write(out_dir.join("dict.tsv"), tsv)?;
    tracing::info!(
        added_words,
        added_priors,
        "人工补词已写出，已有词频与二元计数保留"
    );
    Ok(())
}

fn read_words(path: &Path) -> Result<BTreeMap<(String, String), u32>, ConvertError> {
    let mut words = BTreeMap::new();
    let mut seen = HashSet::new();
    for (index, raw) in std::fs::read_to_string(path)?.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<_> = line.split('\t').collect();
        let invalid = || {
            ConvertError::Format {
            path: path.to_owned(), line: index + 1,
            reason: "expected word<TAB>positive initial weight<TAB>full pinyin, one syllable per character".to_owned(),
        }
        };
        if fields.len() != 3 {
            return Err(invalid());
        }
        let word = fields[0].trim();
        let count = fields[1]
            .trim()
            .parse::<u32>()
            .ok()
            .filter(|n| *n > 0)
            .ok_or_else(invalid)?;
        let syllables: Vec<_> = fields[2]
            .split_whitespace()
            .map(canonical_syllable)
            .collect();
        if word.is_empty()
            || word.chars().any(char::is_whitespace)
            || syllables.len() != word.chars().count()
            || !syllables.iter().all(|s| is_syllable(s))
        {
            return Err(invalid());
        }
        let key = (word.to_owned(), syllables.join(" "));
        if !seen.insert(key.clone()) {
            return Err(invalid());
        }
        words.insert(key, count);
    }
    if words.is_empty() {
        return Err(ConvertError::Format {
            path: path.to_owned(),
            line: 0,
            reason: "empty supplemental word list".to_owned(),
        });
    }
    Ok(words)
}

fn supplemented_metadata(
    base: Option<&Metadata>,
    words: &Path,
    license: &str,
    added: usize,
) -> Metadata {
    let mut meta = base.cloned().unwrap_or_default();
    if added > 0 {
        meta.license = if meta.license.is_empty() {
            license.to_owned()
        } else {
            format!("({}) AND ({license})", meta.license)
        };
        let name = words.file_name().unwrap_or_default().to_string_lossy();
        meta.attribution
            .push_str(&format!("；人工补词：{name}（初始化权重，非语料计数）"));
    }
    meta.generator = format!(
        "qingjian-dict-convert {} supplement",
        env!("CARGO_PKG_VERSION")
    );
    meta
}
