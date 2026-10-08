//! 批量保存的结果：要么整体写入成功，要么一个字段都不写、把冲突列出来。

use super::conflict::FieldConflict;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplyOutcome {
    /// 已原子写入；调用方重新读快照即可继续编辑。
    Applied,

    /// 有字段与外部修改冲突，整体未写入。
    Conflicts(Vec<FieldConflict>),
}

impl ApplyOutcome {
    /// 是否写入成功（无冲突）。
    pub fn applied(&self) -> bool {
        matches!(self, Self::Applied)
    }
}
