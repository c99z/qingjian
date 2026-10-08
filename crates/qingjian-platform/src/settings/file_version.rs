//! 设置编辑器读到的配置文件版本：mtime + 长度 + 内容 FNV-1a 哈希。
//! 编辑期间拿它判定文件有没有被外部改过；三路比较以字段内容为准，版本只做整体变化的快速判断。

use std::time::SystemTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileVersion {
    /// 文件字节数。
    pub len: u64,

    /// 修改时间；读不到（例如权限）为 `None`。
    pub modified: Option<SystemTime>,

    /// 内容的 FNV-1a 哈希：就地改写但长度与 mtime 都没变的情况靠它兜底。
    pub hash: u64,
}

impl FileVersion {
    /// 从已读出的原文算版本，避免再读一次盘。
    pub fn from_source(source: &str, modified: Option<SystemTime>) -> Self {
        Self {
            len: source.len() as u64,
            modified,
            hash: fnv1a(source.as_bytes()),
        }
    }
}

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_source_same_version_different_source_different_hash() {
        let a = FileVersion::from_source("page_size = 5\n", None);
        let b = FileVersion::from_source("page_size = 5\n", None);
        // 等长不同内容也要能区分
        let c = FileVersion::from_source("page_size = 6\n", None);
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(a.len, 14);
    }
}
