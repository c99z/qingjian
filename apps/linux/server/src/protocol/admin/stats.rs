//! 统计报告：输入量与词汇学习统计的管理协议表达（Core 的汇总类型不带 serde）。
use qingjian_core::{Usage, UsageSummary, VocabularySummary};
use serde::{Deserialize, Serialize};

/// `Stats` 命令的回包。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatsReport {
    /// 输入量统计。
    pub usage: UsageReport,

    /// 当前学习语言的词汇学习统计。
    pub vocabulary: VocabularyReport,
}

/// 输入量：今日、近七天与累计。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsageReport {
    pub today: UsageCounts,

    /// 最近 7 天（含今天）。
    pub week: UsageCounts,

    /// 从有记录起的累计。
    pub total: UsageCounts,

    /// 有过上屏的天数。
    pub days: u32,

    /// 最早一条记录的日期（`YYYY-MM-DD`）；没记录为 `None`。
    pub since: Option<String>,
}

impl From<UsageSummary> for UsageReport {
    fn from(summary: UsageSummary) -> Self {
        Self {
            today: summary.today.into(),
            week: summary.week.into(),
            total: summary.total.into(),
            days: summary.days,
            since: summary.since,
        }
    }
}

/// 一段时间的输入量。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsageCounts {
    pub hanzi: u64,

    pub words: u64,

    pub english_words: u64,

    pub commits: u64,
}

impl From<Usage> for UsageCounts {
    fn from(usage: Usage) -> Self {
        Self {
            hanzi: usage.hanzi,
            words: usage.words,
            english_words: usage.english_words,
            commits: usage.commits,
        }
    }
}

/// 词汇学习统计（等级分布首版不进管理协议）。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VocabularyReport {
    /// 见过的不同译词数。
    pub seen: u64,

    /// 其中看熟了的。
    pub familiar: u64,

    /// 上屏过带这条译词的候选的不同译词数。
    pub committed: u64,

    /// 直接打出过的不同译词数。
    pub used: u64,

    /// 最近 7 天（含今天）第一次见到的译词数。
    pub new_this_week: u64,
}

impl From<VocabularySummary> for VocabularyReport {
    fn from(summary: VocabularySummary) -> Self {
        Self {
            seen: summary.seen,
            familiar: summary.familiar,
            committed: summary.committed,
            used: summary.used,
            new_this_week: summary.new_this_week,
        }
    }
}
