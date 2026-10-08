//! 脱敏、路径边界和恢复事务的验证。
use super::{Archive, BackupPaths, RestoreGuard};
use base64::{Engine, prelude::BASE64_STANDARD};
#[test]
fn backup_excludes_credentials_logs_and_models_and_restore_rolls_back() {
    let dir = tempfile::tempdir().unwrap();
    let paths = BackupPaths {
        config: dir.path().join("config.toml"),
        data: dir.path().join("data"),
        themes: dir.path().join("themes"),
    };
    std::fs::create_dir_all(&paths.data).unwrap();
    std::fs::write(
        &paths.config,
        "nested = { token = 'inline-secret', child = [{ password = 'array-secret', keep = 'ok' }] }\n[[remote]]\nsecret = 'table-secret'\nname = 'keep'\n[predict]\nenabled = true\napi_key = 'secret'\n",
    )
    .unwrap();
    std::fs::write(paths.data.join("user.tsv"), "中文\t3\n").unwrap();
    std::fs::write(paths.data.join(".env"), "secret").unwrap();
    std::fs::write(paths.data.join("input-log.jsonl"), "private").unwrap();
    let archive = Archive::capture(&paths).unwrap();
    assert_eq!(archive.files.len(), 2);
    let config = String::from_utf8(
        BASE64_STANDARD
            .decode(&archive.files["config.toml"])
            .unwrap(),
    )
    .unwrap();
    assert!(!config.contains("secret"));
    assert!(!config.contains("enabled = true"));
    std::fs::write(paths.data.join("user.tsv"), "新的\t7\n").unwrap();
    {
        let _guard = RestoreGuard::prepare(&archive, &paths).unwrap();
        assert_eq!(
            std::fs::read_to_string(paths.data.join("user.tsv")).unwrap(),
            "中文\t3\n"
        );
    }
    assert_eq!(
        std::fs::read_to_string(paths.data.join("user.tsv")).unwrap(),
        "新的\t7\n"
    );
    assert!(
        std::fs::read_to_string(&paths.config)
            .unwrap()
            .contains("enabled = true")
    );
    RestoreGuard::prepare(&archive, &paths).unwrap().commit();
    let config = crate::Config::load(&paths.config).unwrap();
    assert!(!config.predict.enabled);
    assert_eq!(config.predict.api_key.as_deref(), Some("secret"));
}

#[test]
fn restore_removes_new_theme_files_and_rollback_restores_them() {
    let dir = tempfile::tempdir().unwrap();
    let paths = BackupPaths {
        config: dir.path().join("config.toml"),
        data: dir.path().join("data"),
        themes: dir.path().join("themes"),
    };
    let theme = paths.themes.join("qingjian-custom");
    std::fs::create_dir_all(theme.join("images")).unwrap();
    std::fs::write(theme.join("theme.conf"), "original").unwrap();
    let archive = Archive::capture(&paths).unwrap();
    std::fs::write(theme.join("theme.conf"), "modified").unwrap();
    std::fs::write(theme.join("images/new.png"), "extra").unwrap();
    {
        let _guard = RestoreGuard::prepare(&archive, &paths).unwrap();
        assert!(!theme.join("images/new.png").exists());
        assert_eq!(
            std::fs::read_to_string(theme.join("theme.conf")).unwrap(),
            "original"
        );
    }
    assert_eq!(
        std::fs::read_to_string(theme.join("images/new.png")).unwrap(),
        "extra"
    );
    assert_eq!(
        std::fs::read_to_string(theme.join("theme.conf")).unwrap(),
        "modified"
    );
    RestoreGuard::prepare(&archive, &paths).unwrap().commit();
    assert!(!theme.join("images/new.png").exists());
}
#[test]
fn rejects_traversal_secrets_unknown_version_and_symlinks() {
    let dir = tempfile::tempdir().unwrap();
    let paths = BackupPaths {
        config: dir.path().join("config.toml"),
        data: dir.path().join("data"),
        themes: dir.path().join("themes"),
    };
    let mut archive = Archive::capture(&paths).unwrap();
    archive
        .files
        .insert("data/../.env".into(), BASE64_STANDARD.encode("key"));
    assert!(archive.validate().is_err());
    archive.files.remove("data/../.env");
    archive.version = 99;
    assert!(archive.validate().is_err());
    archive.version = 1;
    archive.files.insert(
        "config.toml".into(),
        BASE64_STANDARD.encode("[predict]\napi_key = 'secret'\n"),
    );
    assert!(archive.validate().is_err());
    #[cfg(unix)]
    {
        let archive = Archive::capture(&paths).unwrap();
        let outside = dir.path().join("outside");
        std::fs::create_dir(&outside).unwrap();
        std::os::unix::fs::symlink(&outside, &paths.data).unwrap();
        assert!(RestoreGuard::prepare(&archive, &paths).is_err());
        assert!(std::fs::read_dir(outside).unwrap().next().is_none());
    }
}

#[test]
fn read_rejects_valid_base64_with_wrong_checksum() {
    let dir = tempfile::tempdir().unwrap();
    let paths = BackupPaths {
        config: dir.path().join("config.toml"),
        data: dir.path().join("data"),
        themes: dir.path().join("themes"),
    };
    let mut archive = Archive::capture(&paths).unwrap();
    archive.files.insert(
        "config.toml".into(),
        BASE64_STANDARD.encode("[general]\npage_size=3\n"),
    );
    let path = dir.path().join("tampered.qjb");
    std::fs::write(&path, serde_json::to_vec(&archive).unwrap()).unwrap();
    assert!(Archive::read(&path).is_err());
}
