//! 凭据优先级、权限和无环境插值验证。
use super::{read_key, resolve, write_key};
use qingjian_predict::PredictConfig;
#[test]
fn local_key_is_private_and_preserves_other_entries() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(".env");
    std::fs::write(&path, "# comment\nOTHER='keep'\nQINGJIAN_API_KEY='old'\n").unwrap();
    write_key(&path, "QINGJIAN_API_KEY", "sk-$literal").unwrap();
    assert_eq!(
        read_key(&path, "QINGJIAN_API_KEY").unwrap().as_deref(),
        Some("sk-$literal")
    );
    assert!(
        std::fs::read_to_string(&path)
            .unwrap()
            .contains("OTHER='keep'")
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    let config = PredictConfig::default();
    assert_eq!(
        resolve(&config, &dir.path().join("config.toml"))
            .unwrap()
            .api_key
            .as_deref(),
        Some("sk-$literal")
    );
    let config = PredictConfig {
        api_key: Some("explicit".into()),
        ..config
    };
    assert_eq!(
        resolve(&config, &dir.path().join("config.toml"))
            .unwrap()
            .api_key
            .as_deref(),
        Some("explicit")
    );
    assert!(write_key(&path, "QINGJIAN_API_KEY", "bad\nkey").is_err());
}
