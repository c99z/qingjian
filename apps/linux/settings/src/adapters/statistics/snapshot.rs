//! 统计值及其来源，离线读取时标明落盘文件更新时间。

use qingjian_platform::protocol::linux_admin::StatsReport;

#[derive(Debug)]
pub struct StatisticsSnapshot {
    pub report: StatsReport,

    pub source: String,
}
