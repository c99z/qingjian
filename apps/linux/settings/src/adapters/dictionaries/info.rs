//! 设置页显示的词库信息。
use std::path::PathBuf;
pub struct DictionaryInfo {
    pub stem: String,

    pub path: PathBuf,

    pub bundled: bool,

    pub description: String,
}
