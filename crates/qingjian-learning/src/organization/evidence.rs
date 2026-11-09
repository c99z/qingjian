//! 可发送给整理模型的最小证据，不包含原始按键、应用名或整句上下文。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WordEvidence {
    pub word: String,

    pub pinyin: String,

    pub count: u32,

    pub days: usize,
}
