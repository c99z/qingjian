//! 离线统计使用既有数据，服务故障不会伪装成离线零值。

use super::load;
use crate::adapters::admin::AdminClient;

#[test]
fn offline_stats_use_saved_records_and_report_their_age() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("usage.tsv"), "2026-09-01\t12\t4\t3\t7\n").unwrap();
    let admin = AdminClient::with_path(dir.path().join("missing.sock"));
    let snapshot = load(&admin, dir.path(), "off").unwrap();
    assert_eq!(snapshot.report.usage.total.hanzi, 12);
    assert_eq!(snapshot.report.usage.total.commits, 7);
    assert_eq!(snapshot.report.vocabulary.seen, 0);
    assert!(snapshot.source.contains("更新于"));
}

#[test]
fn a_fresh_account_has_no_saved_stats() {
    let dir = tempfile::tempdir().unwrap();
    let admin = AdminClient::with_path(dir.path().join("missing.sock"));
    assert!(
        load(&admin, dir.path(), "en")
            .unwrap()
            .source
            .contains("暂无")
    );
}
