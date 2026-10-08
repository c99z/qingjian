//! 密钥草稿保存、配置冲突回滚和外部凭据冲突保护。
use crate::adapters::admin::AdminClient;
use crate::state::{DraftStore, SaveOutcome, save_with_credential};
#[test]
fn keys_are_private_and_config_conflicts_restore_old_credentials() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let key = dir.path().join(".env");
    std::fs::write(&path, "[general]\npage_size=9 # keep\n").unwrap();
    std::fs::write(&key, "QINGJIAN_API_KEY='old'\n").unwrap();
    let mut store = DraftStore::load(&path).unwrap();
    store.set_credential("new-secret".into());
    store.set("general", "page_size", 5i64);
    std::fs::write(&path, "[general]\npage_size=7\n").unwrap();
    let admin = AdminClient::with_path(dir.path().join("missing.sock"));
    let outcome = save_with_credential(
        store.path(),
        store.snapshot(),
        store.patch(),
        &admin,
        None,
        store.credential(),
    );
    assert!(matches!(outcome, SaveOutcome::Conflicts(_)));
    assert!(std::fs::read_to_string(&key).unwrap().contains("old"));
    store.reload().unwrap();
    store.set_credential("new-secret".into());
    let outcome = save_with_credential(
        store.path(),
        store.snapshot(),
        store.patch(),
        &admin,
        None,
        store.credential(),
    );
    assert!(matches!(outcome, SaveOutcome::Saved { .. }));
    assert!(
        !std::fs::read_to_string(&path)
            .unwrap()
            .contains("new-secret")
    );
    assert!(
        std::fs::read_to_string(&key)
            .unwrap()
            .contains("new-secret")
    );
}
#[test]
fn external_credential_changes_are_never_overwritten() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    let key = dir.path().join(".env");
    std::fs::write(&path, "[general]\npage_size=9\n").unwrap();
    std::fs::write(&key, "QINGJIAN_API_KEY='old'\n").unwrap();
    let mut store = DraftStore::load(&path).unwrap();
    store.set_credential("draft-key".into());
    std::fs::write(&key, "QINGJIAN_API_KEY='external-key'\n").unwrap();
    let admin = AdminClient::with_path(dir.path().join("missing.sock"));
    assert!(matches!(
        save_with_credential(
            store.path(),
            store.snapshot(),
            store.patch(),
            &admin,
            None,
            store.credential()
        ),
        SaveOutcome::WriteFailed(_)
    ));
    assert!(
        std::fs::read_to_string(&key)
            .unwrap()
            .contains("external-key")
    );
    assert!(!std::fs::read_to_string(&path).unwrap().contains("api_key"));
}
