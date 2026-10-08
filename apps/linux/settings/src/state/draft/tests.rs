//! 草稿累积、改回原值、撤销与重新载入的行为。

use std::path::PathBuf;

use qingjian_platform::settings::SettingsSnapshot;

use super::DraftStore;

fn temp_config(name: &str, content: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("qingjian-draft-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    std::fs::write(&path, content).unwrap();
    path
}

#[test]
fn edits_accumulate_and_the_last_write_wins() {
    let path = temp_config("accumulate.toml", "[general]\npage_size = 9\n");
    let mut store = DraftStore::load(&path).unwrap();
    assert!(!store.is_dirty());
    store.set("general", "page_size", 5i64);
    store.set("general", "preedit", "window");
    store.set("general", "page_size", 7i64);
    assert!(store.is_dirty());
    assert_eq!(store.patch().edits.len(), 2, "同字段只留最后一次");
    let draft = store.draft().unwrap();
    assert_eq!(draft.general.page_size, 7);
    assert_eq!(
        draft.general.preedit,
        qingjian_platform::PreeditMode::Window
    );
}

#[test]
fn editing_back_to_the_snapshot_value_is_not_dirty() {
    let path = temp_config("roundtrip.toml", "[general]\npage_size = 9\n");
    let mut store = DraftStore::load(&path).unwrap();
    store.set("general", "page_size", 5i64);
    assert!(store.is_dirty());
    store.set("general", "page_size", 9i64);
    assert!(!store.is_dirty(), "改回原值应撤销这条编辑");
    assert!(store.patch().is_empty());
}

#[test]
fn revert_goes_back_to_the_snapshot() {
    let path = temp_config("revert.toml", "[general]\npage_size = 9\n");
    let mut store = DraftStore::load(&path).unwrap();
    store.set("general", "page_size", 3i64);
    store.set("model", "enabled", false);
    store.revert();
    assert!(!store.is_dirty());
    assert_eq!(store.draft().unwrap(), store.config().clone());
}

#[test]
fn reload_discards_the_draft_and_picks_up_external_changes() {
    let path = temp_config("reload.toml", "[general]\npage_size = 9\n");
    let mut store = DraftStore::load(&path).unwrap();
    store.set("general", "page_size", 3i64);
    std::fs::write(&path, "[general]\npage_size = 6\npreedit = \"inline\"\n").unwrap();
    store.reload().unwrap();
    assert!(!store.is_dirty());
    assert_eq!(store.config().general.page_size, 6);
    assert_eq!(
        store.config().general.preedit,
        qingjian_platform::PreeditMode::Inline
    );
}

#[test]
fn a_missing_file_starts_from_defaults() {
    let path = std::env::temp_dir().join(format!(
        "qingjian-draft-missing-{}/config.toml",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    let snapshot = SettingsSnapshot::load(&path).unwrap();
    let store = DraftStore::new(snapshot);
    assert!(!store.is_dirty());
    assert_eq!(store.config(), &qingjian_platform::Config::default());
}
