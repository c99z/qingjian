//! 共享设置基础（P1）的集成测试：快照、校验、三路比较批量保存与资源枚举。

use std::path::{Path, PathBuf};

use qingjian_core::{CustomPhrase, ShuangpinScheme};
use qingjian_platform::dirs;
use qingjian_platform::settings::{
    self, ApplyError, ApplyOutcome, ApplyReport, ComponentStatus, ConfigPatch, SettingsSnapshot,
};
use qingjian_platform::{Config, ThemeMode};

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "qingjian-settings-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn snapshot(path: &Path) -> SettingsSnapshot {
    SettingsSnapshot::load(path).unwrap()
}

#[test]
fn partial_file_falls_back_to_defaults() {
    let dir = temp_dir("partial");
    let path = dir.join("config.toml");
    std::fs::write(&path, "[predict]\nenabled = true\n").unwrap();
    let snapshot = snapshot(&path);
    assert!(snapshot.config.predict.enabled);
    assert_eq!(snapshot.config.general.page_size(), 9);
    assert!(snapshot.parse_error.is_none());
    assert!(snapshot.version.is_some());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn old_config_keys_still_parse() {
    let dir = temp_dir("legacy");
    let path = dir.join("config.toml");
    std::fs::write(&path, "[general]\nshuangpin = \"xiaohe\"\n").unwrap();
    let snapshot = snapshot(&path);
    assert_eq!(
        snapshot.config.general.shuangpin(),
        Some(ShuangpinScheme::Xiaohe)
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn apply_preserves_unknown_fields_comments_and_order() {
    let dir = temp_dir("preserve");
    let path = dir.join("config.toml");
    let original = "# 顶部注释\n\
                    [general]\n\
                    # 每页候选数（1–9）\n\
                    page_size = 5 # 行尾注释\n\
                    unknown_key = \"keep me\"\n\
                    theme = \"dark\"\n\
                    \n\
                    [future_section]\n\
                    anything = [1, 2]\n";
    std::fs::write(&path, original).unwrap();
    let snapshot = snapshot(&path);
    let mut patch = ConfigPatch::new();
    patch.set("general", "page_size", 7i64);
    let outcome = settings::apply(&path, &snapshot, &patch).unwrap();
    assert_eq!(outcome, ApplyOutcome::Applied);

    let saved = std::fs::read_to_string(&path).unwrap();
    let before: Vec<&str> = original.lines().collect();
    let after: Vec<&str> = saved.lines().collect();
    assert_eq!(before.len(), after.len(), "只许改一行：\n{saved}");
    for (old, new) in before.iter().zip(&after) {
        if old.starts_with("page_size") {
            assert_eq!(*new, "page_size = 7 # 行尾注释");
        } else {
            assert_eq!(old, new);
        }
    }
    assert_eq!(Config::load(&path).unwrap().general.page_size(), 7);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn validate_rejects_illegal_draft_with_field_paths() {
    let dir = temp_dir("validate");
    let path = dir.join("config.toml");
    let snapshot = snapshot(&path); // 文件不存在：从模板起步
    let mut patch = ConfigPatch::new();
    patch
        .set("general", "page_size", 42i64)
        .set("general", "page_keys", "ab")
        .set("general", "scheme", "flypy")
        .set("shortcut", "expression", "x")
        .set("shortcut", "translation", "option")
        .set("shortcut", "translation_second", "option")
        .set("shortcut", "delete_candidate", "option")
        .set("predict", "timeout_ms", 10i64)
        .set("predict", "debounce_ms", 99999i64)
        .set("predict", "slots", 42i64);
    let draft = patch.draft(&snapshot).unwrap();
    let errors = settings::validate(&draft);
    let fields: Vec<&str> = errors.iter().map(|e| e.field.as_str()).collect();
    for expected in [
        "general.page_size",
        "general.page_keys",
        "general.scheme",
        "shortcut.expression",
        "shortcut.translation_second",
        "shortcut.delete_candidate",
        "predict.timeout_ms",
        "predict.debounce_ms",
        "predict.slots",
    ] {
        assert!(fields.contains(&expected), "{expected} 不在 {fields:?}");
    }
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn conflicting_external_edit_blocks_the_whole_write() {
    let dir = temp_dir("conflict");
    let path = dir.join("config.toml");
    std::fs::write(&path, "[general]\npage_size = 5\ntheme = \"dark\"\n").unwrap();
    let snapshot = snapshot(&path);
    // 外部（手改 / 另一个入口）把同一个字段改成别的值
    std::fs::write(&path, "[general]\npage_size = 6\ntheme = \"dark\"\n").unwrap();
    let mut patch = ConfigPatch::new();
    patch.set("general", "page_size", 7i64);
    let outcome = settings::apply(&path, &snapshot, &patch).unwrap();
    let ApplyOutcome::Conflicts(conflicts) = outcome else {
        panic!("应该是冲突：{outcome:?}");
    };
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].field, "general.page_size");
    assert_eq!(conflicts[0].base.as_deref(), Some("5"));
    assert_eq!(conflicts[0].current.as_deref(), Some("6"));
    assert_eq!(conflicts[0].draft, "7");
    // 整体拒绝：文件保持外部改后的样子
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "[general]\npage_size = 6\ntheme = \"dark\"\n"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn broken_file_is_never_overwritten() {
    let dir = temp_dir("broken");
    let path = dir.join("config.toml");
    let broken = "[general\npage_size = 5\n";
    std::fs::write(&path, broken).unwrap();
    let snapshot = snapshot(&path);
    assert!(snapshot.parse_error.is_some());
    assert_eq!(snapshot.config, Config::default());
    let mut patch = ConfigPatch::new();
    patch.set("general", "page_size", 7i64);
    let error = settings::apply(&path, &snapshot, &patch).unwrap_err();
    assert!(matches!(error, ApplyError::Broken { .. }), "{error}");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), broken);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn untouched_fields_follow_external_changes() {
    let dir = temp_dir("follow");
    let path = dir.join("config.toml");
    std::fs::write(&path, "[general]\npage_size = 5\ntheme = \"system\"\n").unwrap();
    let snapshot = snapshot(&path);
    // 外部改了草稿没碰的字段
    Config::set_value(&path, "general", "theme", "dark").unwrap();
    let mut patch = ConfigPatch::new();
    patch.set("general", "page_size", 7i64);
    let outcome = settings::apply(&path, &snapshot, &patch).unwrap();
    assert_eq!(outcome, ApplyOutcome::Applied);
    let config = Config::load(&path).unwrap();
    assert_eq!(config.general.page_size(), 7);
    assert_eq!(config.general.theme, ThemeMode::Dark);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn file_version_changes_with_the_content() {
    let dir = temp_dir("version");
    let path = dir.join("config.toml");
    std::fs::write(&path, "[general]\npage_size = 5\n").unwrap();
    let before = snapshot(&path).version.unwrap();
    // 等长改写：mtime 可能相同，内容哈希必须能区分
    std::fs::write(&path, "[general]\npage_size = 6\n").unwrap();
    let after = snapshot(&path).version.unwrap();
    assert_ne!(before, after);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn convergent_external_edit_is_not_a_conflict() {
    let dir = temp_dir("converge");
    let path = dir.join("config.toml");
    std::fs::write(&path, "[general]\npage_size = 5\n").unwrap();
    let snapshot = snapshot(&path);
    Config::set_value(&path, "general", "page_size", 7i64).unwrap();
    let mut patch = ConfigPatch::new();
    patch.set("general", "page_size", 7i64);
    let outcome = settings::apply(&path, &snapshot, &patch).unwrap();
    assert_eq!(outcome, ApplyOutcome::Applied);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn custom_phrases_conflict_when_edited_externally() {
    let dir = temp_dir("phrases");
    let path = dir.join("config.toml");
    std::fs::write(
        &path,
        "[[custom_phrases]]\ncode = \"aa\"\ntext = \"甲\"\nposition = 1\nenabled = true\n",
    )
    .unwrap();
    let snapshot = snapshot(&path);
    // 外部把短语改了
    let mut external = snapshot.config.custom_phrases.clone();
    external[0].text = "乙".into();
    Config::set_custom_phrases(&path, &external).unwrap();
    // 草稿也想改同一份短语列表
    let mut draft = snapshot.config.custom_phrases.clone();
    draft[0].text = "丙".into();
    let mut patch = ConfigPatch::new();
    patch.set_custom_phrases(draft);
    let outcome = settings::apply(&path, &snapshot, &patch).unwrap();
    let ApplyOutcome::Conflicts(conflicts) = outcome else {
        panic!("应该是冲突：{outcome:?}");
    };
    assert_eq!(conflicts[0].field, "custom_phrases");
    assert_eq!(Config::load(&path).unwrap().custom_phrases[0].text, "乙");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn invalid_phrases_in_the_patch_are_rejected_before_writing() {
    let dir = temp_dir("phrases-invalid");
    let path = dir.join("config.toml");
    let snapshot = snapshot(&path);
    let phrase = CustomPhrase {
        code: "aa".into(),
        text: "甲".into(),
        position: 1,
        enabled: true,
    };
    let mut patch = ConfigPatch::new();
    patch.set_custom_phrases(vec![phrase.clone(), phrase]);
    let error = settings::apply(&path, &snapshot, &patch).unwrap_err();
    assert!(matches!(error, ApplyError::Phrases(_)), "{error}");
    assert!(!path.exists());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn apply_report_serializes_for_the_management_socket() {
    let report = ApplyReport {
        server: ComponentStatus::Saved,
        fcitx5: ComponentStatus::Loading,
    };
    let json = serde_json::to_string(&report).unwrap();
    assert_eq!(serde_json::from_str::<ApplyReport>(&json).unwrap(), report);
}

#[test]
fn resource_discovery_lists_dicts_glossaries_and_models() {
    let dir = temp_dir("resources");
    let root = dir.join("resources");
    let user = dir.join("user");
    std::fs::create_dir_all(root.join("data/generated/dicts")).unwrap();
    std::fs::create_dir_all(root.join("data/models/hanzhang-zhiwei")).unwrap();
    std::fs::create_dir_all(user.join("dicts")).unwrap();
    std::fs::write(root.join("data/generated/dicts/idioms.qj"), b"").unwrap();
    std::fs::write(root.join("data/generated/dicts/law.tsv"), b"").unwrap();
    std::fs::write(root.join("data/generated/glossary-en.qj"), b"").unwrap();
    std::fs::write(user.join("dicts/mine.qj"), b"").unwrap();

    let found = settings::ResourceCapabilities::discover(Some(&root), &user);
    assert_eq!(found.bundled_domains, ["idioms", "law"]);
    assert_eq!(found.glossary_languages, ["en"]);
    assert_eq!(found.user_dictionaries.len(), 1);
    assert_eq!(found.user_dictionaries[0].name, "mine");
    assert!(found.user_dictionaries[0].exists);
    let bundled_model = found
        .model_candidates
        .iter()
        .find(|f| f.path.starts_with(&root) && f.exists)
        .unwrap();
    assert_eq!(bundled_model.name, "hanzhang-zhiwei");
    assert!(found.model_candidates.iter().any(|f| !f.exists));

    // 没有资源根时随包部分为空，用户部分照常
    let bare = settings::ResourceCapabilities::discover(None, &user);
    assert!(bare.bundled_domains.is_empty());
    assert_eq!(bare.user_dictionaries.len(), 1);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn linux_dirs_point_into_the_xdg_layout() {
    // 不碰环境变量：只看路径结尾是否符合 XDG 约定
    assert!(dirs::linux::config_path().ends_with("qingjian/config.toml"));
    assert!(dirs::linux::log_dir().ends_with("qingjian/logs"));
    assert!(dirs::linux::user_dir().ends_with("qingjian"));
}
