//! 主题复制与 `theme.conf` 改写的单测：临时目录造假主题（含图片文件），验证整体复制与键改写。

use super::{ThemePatch, patch_theme_conf};

fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir =
        std::env::temp_dir().join(format!("qingjian-theme-test-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

const SAMPLE_CONF: &str = "[Metadata]\nName=macOS-light\nName[zh_CN]=类 macOS\nVersion=0.1\n\n\
    [InputPanel]\nNormalColor=#000000\nCandidateCommentColor=#164B73\nCommentTextSizeFactor=100\n\n\
    [InputPanel/Background]\nImage=panel.svg\nColor=#ffffff\n";

#[test]
fn patch_replaces_metadata_and_overwrites_existing_keys() {
    let patch = ThemePatch {
        comment_color: Some("#112233".to_owned()),
        highlight_comment_color: Some("#ffffff".to_owned()),
        comment_size_factor: Some(80),
        ..ThemePatch::default()
    };
    let out = patch_theme_conf(SAMPLE_CONF, &patch);
    // Metadata 整段替换，原来的 Name[zh_CN] 不残留
    assert!(out.contains("Name=Qingjian Custom"));
    assert!(out.contains("Name[zh_CN]=青简定制"));
    assert!(!out.contains("macOS-light"));
    // 已有键就地覆盖，没补丁的键原样保留
    assert!(out.contains("[InputPanel/CandidateCommentColor]\nValue=#112233"));
    assert!(out.contains("[InputPanel/HighlightCandidateCommentColor]\nValue=#ffffff"));
    assert!(out.contains("CommentTextSizeFactor=80"));
    assert!(out.contains("NormalColor=#000000"));
    assert!(out.contains("Image=panel.svg"));
    // 每个键只出现一次（HighlightCandidateCommentColor 包含 CandidateCommentColor 子串，按行首数）
    assert_eq!(out.matches("[InputPanel/CandidateCommentColor]").count(), 1);
    assert_eq!(out.matches("[Metadata]").count(), 1);
}

#[test]
fn patch_appends_missing_keys_to_input_panel() {
    let conf = "[Metadata]\nName=x\n\n[InputPanel]\nNormalColor=#000000\n";
    let patch = ThemePatch {
        comment_color: Some("#445566".to_owned()),
        ..ThemePatch::default()
    };
    let out = patch_theme_conf(conf, &patch);
    let input_panel = out.split("[InputPanel]").nth(1).unwrap();
    assert!(input_panel.contains("[InputPanel/CandidateCommentColor]\nValue=#445566"));
    assert!(input_panel.contains("NormalColor=#000000"));
}

#[test]
fn patch_without_input_panel_section_appends_one() {
    let conf = "[Metadata]\nName=x\n\n[Menu]\nSpacing=0\n";
    let patch = ThemePatch {
        comment_size_factor: Some(100),
        ..ThemePatch::default()
    };
    let out = patch_theme_conf(conf, &patch);
    assert!(out.contains("[InputPanel]\nCommentTextSizeFactor=100\n"));
    assert!(out.contains("[Menu]"));
}

#[test]
fn empty_patch_only_replaces_metadata() {
    let out = patch_theme_conf(SAMPLE_CONF, &ThemePatch::default());
    assert!(out.contains("CandidateCommentColor=#164B73"));
    assert!(out.contains("Name=Qingjian Custom"));
}

#[test]
fn copy_custom_theme_copies_images_and_rewrites_conf() {
    let dir = temp_dir("copy");
    let source = dir.join("source-theme");
    std::fs::create_dir_all(&source).unwrap();
    std::fs::write(source.join("theme.conf"), SAMPLE_CONF).unwrap();
    std::fs::write(source.join("panel.svg"), "<svg/>").unwrap();
    std::fs::write(source.join("highlight.svg"), "<svg/>").unwrap();

    let home = dir.join("home");
    let themes = home.join("fcitx5/themes");
    std::fs::create_dir_all(&themes).unwrap();
    let linked = themes.join("source-theme");
    copy_tree_for_test(&source, &linked);
    let patch = ThemePatch {
        comment_color: Some("#102030".to_owned()),
        ..ThemePatch::default()
    };
    let target = themes.join(super::CUSTOM_THEME_NAME);
    super::ThemeTransaction::at(&linked, &target, &patch)
        .unwrap()
        .commit();
    assert_eq!(target, themes.join(super::CUSTOM_THEME_NAME));
    assert!(target.join("panel.svg").is_file(), "图片资源要一起复制");
    assert!(target.join("highlight.svg").is_file());
    let conf = std::fs::read_to_string(target.join("theme.conf")).unwrap();
    assert!(conf.contains("Name=Qingjian Custom"));
    assert!(conf.contains("[InputPanel/CandidateCommentColor]\nValue=#102030"));
    assert!(conf.contains("NormalColor=#000000"));
    // 源主题没被改动
    assert_eq!(
        std::fs::read_to_string(linked.join("theme.conf")).unwrap(),
        SAMPLE_CONF
    );

    // 换源重定制：先清掉旧目录再复制，不残留上一套的文件
    let other = dir.join("other-theme");
    std::fs::create_dir_all(&other).unwrap();
    std::fs::write(
        other.join("theme.conf"),
        "[Metadata]\nName=o\n[InputPanel]\n",
    )
    .unwrap();
    copy_tree_for_test(&other, &themes.join("other-theme"));
    super::ThemeTransaction::at(&themes.join("other-theme"), &target, &ThemePatch::default())
        .unwrap()
        .commit();
    assert!(
        !target.join("panel.svg").exists(),
        "换源后旧主题的图片不应残留"
    );

    // source = None：就地改写现有定制主题
    let repatch = ThemePatch {
        comment_size_factor: Some(90),
        ..ThemePatch::default()
    };
    super::ThemeTransaction::at(&target, &target, &repatch)
        .unwrap()
        .commit();
    let conf = std::fs::read_to_string(target.join("theme.conf")).unwrap();
    assert!(conf.contains("CommentTextSizeFactor=90"));

    let _ = std::fs::remove_dir_all(&dir);
}

fn copy_tree_for_test(source: &std::path::Path, target: &std::path::Path) {
    std::fs::create_dir_all(target).unwrap();
    for entry in std::fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        std::fs::copy(entry.path(), target.join(entry.file_name())).unwrap();
    }
}

#[test]
fn theme_options_keep_first_then_presets_then_the_rest() {
    let themes = vec![
        super::InstalledTheme {
            name: "default".to_owned(),
            label: "Default".to_owned(),
        },
        super::InstalledTheme {
            name: "default-dark".to_owned(),
            label: "Default Dark".to_owned(),
        },
        super::InstalledTheme {
            name: "macOS-light".to_owned(),
            label: "类 macOS".to_owned(),
        },
    ];
    let options = super::theme_options(&themes, "macOS-light");
    assert_eq!(options[0].name, None);
    assert!(options[0].label.contains("类 macOS"));
    assert_eq!(options[1].name.as_deref(), Some("default"));
    assert_eq!(options[2].name.as_deref(), Some("default-dark"));
    assert_eq!(options[3].name.as_deref(), Some("macOS-light"));
    assert_eq!(options.len(), 4);

    // 没装 default-dark 时不列深色预设
    let options = super::theme_options(&themes[..1], "default");
    assert_eq!(options.len(), 2);
}

#[test]
fn failed_preparation_and_rollback_preserve_the_original_theme() {
    let dir = temp_dir("rollback");
    let target = dir.join("target");
    std::fs::create_dir_all(&target).unwrap();
    std::fs::write(target.join("theme.conf"), SAMPLE_CONF).unwrap();
    std::fs::write(target.join("original.svg"), "original").unwrap();
    assert!(
        super::ThemeTransaction::at(&dir.join("missing"), &target, &ThemePatch::default()).is_err()
    );
    assert_eq!(
        std::fs::read_to_string(target.join("theme.conf")).unwrap(),
        SAMPLE_CONF
    );
    let transaction = super::ThemeTransaction::at(
        &target,
        &target,
        &ThemePatch {
            comment_size_factor: Some(60),
            ..ThemePatch::default()
        },
    )
    .unwrap();
    assert!(
        std::fs::read_to_string(target.join("theme.conf"))
            .unwrap()
            .contains("CommentTextSizeFactor=60")
    );
    transaction.rollback().unwrap();
    assert_eq!(
        std::fs::read_to_string(target.join("theme.conf")).unwrap(),
        SAMPLE_CONF
    );
    assert_eq!(
        std::fs::read_to_string(target.join("original.svg")).unwrap(),
        "original"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn theme_names_cannot_escape_the_theme_directory() {
    for name in ["../config", "/etc", "a/b", "..", "", "a\nFont=Sans"] {
        assert!(!super::valid_theme_name(name));
        assert!(super::find_theme_dir(name).is_none());
    }
}
