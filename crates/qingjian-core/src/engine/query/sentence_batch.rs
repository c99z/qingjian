//! 待合并的整句及其插入锚点：整词胜出时，整句仍保留在该词之后。

use crate::candidate::Candidate;

#[derive(Default)]
pub(super) struct SentenceBatch {
    pub(super) items: Vec<Candidate>,

    pub(super) after: Option<String>,
}

impl SentenceBatch {
    pub(super) fn insertion_index(&self, items: &[Candidate], fallback: usize) -> usize {
        self.after
            .as_ref()
            .and_then(|text| items.iter().position(|c| &c.text == text))
            .map_or(fallback, |index| (index + 1).max(fallback))
    }
}
