//! 优先读取服务内存统计；服务未运行才使用本机落盘记录，不装载输入引擎。

mod snapshot;
pub use snapshot::StatisticsSnapshot;

use std::path::Path;

use qingjian_core::Language;
use qingjian_learning::{UsageStats, VocabularyBook};
use qingjian_platform::protocol::linux_admin::{StatsReport, VocabularyReport};

use crate::adapters::admin::{AdminClient, AdminError};

pub fn load(
    admin: &AdminClient,
    user_dir: &Path,
    language: &str,
) -> Result<StatisticsSnapshot, String> {
    match admin.stats() {
        Ok(report) => Ok(StatisticsSnapshot {
            report,
            source: "青简服务的最新统计".to_owned(),
        }),
        Err(AdminError::NotRunning) => offline(user_dir, language),
        Err(error) => Err(error.user_message()),
    }
}

fn offline(user_dir: &Path, language: &str) -> Result<StatisticsSnapshot, String> {
    let paths = [user_dir.join("usage.tsv"), user_dir.join("user-vocab.tsv")];
    // 不把权限错误伪装成从未输入；缺文件才按尚无落盘数据处理。
    for path in &paths {
        if let Err(error) = std::fs::read(path)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            return Err(format!("读取统计失败（{}）：{error}", path.display()));
        }
    }
    let today = jiff::Zoned::now().date();
    let usage = UsageStats::open(&paths[0]).summary_on(today).into();
    let vocabulary = if language == "off" {
        VocabularyReport::default()
    } else {
        VocabularyBook::open(&paths[1])
            .summary_on(language.parse().unwrap_or(Language::English), today)
            .into()
    };
    let updated = paths
        .iter()
        .filter_map(|path| std::fs::metadata(path).ok()?.modified().ok())
        .max()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .and_then(|duration| gtk::glib::DateTime::from_unix_local(duration.as_secs() as i64).ok())
        .and_then(|date| date.format("%Y-%m-%d %H:%M:%S").ok());
    let source = updated.map_or_else(
        || "服务未运行，暂无已保存统计".to_owned(),
        |time| format!("服务未运行；已保存数据更新于 {time}"),
    );
    Ok(StatisticsSnapshot {
        report: StatsReport { usage, vocabulary },
        source,
    })
}

#[cfg(test)]
mod tests;
