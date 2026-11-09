//! 调度窗口、跨重启防重、关闭时迟到结果和组句结束后的安装。

use std::sync::atomic::Ordering;
use std::sync::mpsc;

use super::due;
use crate::dispatch::{ReloadDirs, Router, RouterConfig};
use qingjian_core::Engine;
use qingjian_dictionary::Dictionary;
use qingjian_learning::organization::OrganizationDraft;
use qingjian_platform::Config;

#[path = "../../../tests/cloud/mock.rs"]
#[allow(dead_code)]
mod mock;

#[test]
fn time_window_does_not_repeat_or_catch_up_in_daytime() {
    let before = "2026-11-06T02:59:00+08:00[Asia/Shanghai]".parse().unwrap();
    let during = "2026-11-06T03:00:00+08:00[Asia/Shanghai]".parse().unwrap();
    let late = "2026-11-06T07:00:00+08:00[Asia/Shanghai]".parse().unwrap();
    assert!(!due(3, 0, &before, None));
    assert!(due(3, 0, &during, None));
    assert!(!due(3, 0, &during, Some("2026-11-06")));
    assert!(!due(3, 0, &late, None));
    assert!(!due(25, 0, &during, None));
}

#[test]
fn disabling_discards_late_results_without_touching_dictionary() {
    let dict = Dictionary::parse("什么\tshen me\t1000\n").unwrap();
    let mut r = Router::new(Engine::new(dict), RouterConfig::default());
    let dir = std::env::temp_dir().join(format!(
        "qingjian-organizer-disabled-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let config = Config::default();
    r.watch_config(
        &config,
        dir.join("config.toml"),
        ReloadDirs {
            resource_root: dir.clone(),
            user_dir: Some(dir.clone()),
            bundled_dicts: None,
            input_log: Some(dir.join("input-log.jsonl")),
        },
    );
    let (tx, rx) = mpsc::sync_channel(1);
    r.organizer.running = Some(rx);
    r.organizer.cancel.store(true, Ordering::Release);
    tx.send(Ok((
        OrganizationDraft {
            tsv: "录包\tlu bao\t3\n".into(),
            words: 1,
        },
        vec![],
    )))
    .unwrap();
    r.poll_organizer();
    assert!(!dir.join("dicts/personal-agent.tsv").exists());
    assert!(r.organizer.pending.is_none());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn installation_preserves_previous_and_loads_addon() {
    let dir = std::env::temp_dir().join(format!(
        "qingjian-organizer-install-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(dir.join("dicts")).unwrap();
    std::fs::write(dir.join("dicts/personal-agent.tsv"), "焊道\than dao\t3\n").unwrap();
    let mut r = Router::new(
        Engine::new(Dictionary::parse("路霸\tlu bao\t1000\n").unwrap()),
        RouterConfig::default(),
    );
    let config = Config::default();
    r.watch_config(
        &config,
        dir.join("config.toml"),
        ReloadDirs {
            resource_root: dir.clone(),
            user_dir: Some(dir.clone()),
            bundled_dicts: None,
            input_log: None,
        },
    );
    r.organizer.pending = Some((
        OrganizationDraft {
            tsv: "录包\tlu bao\t3\n".into(),
            words: 1,
        },
        vec!["软件开发".into()],
    ));
    r.composed = Some(crate::dispatch::composed::Composed::Raw {
        text: "lu".into(),
        cursor: 2,
    });
    r.poll_organizer();
    assert!(
        std::fs::read_to_string(dir.join("dicts/personal-agent.tsv"))
            .unwrap()
            .contains("焊道")
    );
    assert!(r.organizer.pending.is_some());
    r.composed = None;
    r.poll_organizer();
    assert!(
        std::fs::read_to_string(dir.join("organizer/previous.tsv"))
            .unwrap()
            .contains("焊道")
    );
    for c in "lubao".chars() {
        r.engine.push(c);
    }
    let query = r.engine.query().unwrap();
    assert!(query.candidates.items.iter().any(|c| c.text == "录包"));
    assert_eq!(query.candidates.items[0].text, "路霸");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn worker_uses_real_http_but_only_sends_filtered_evidence() {
    let dir =
        std::env::temp_dir().join(format!("qingjian-organizer-http-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let now = jiff::Timestamp::now().as_second();
    let rows=(0..3).map(|i| serde_json::json!({"event":"commit","id":i+1,"t":jiff::Timestamp::from_second(now-(i+1)*86400).unwrap().to_string(),"text":"录包","scope":"lubao","keys":"lubao","pinyin":"lu'bao","english":false,"source":"sentence","app":"never-send-this-app","top":["陆爆"]}).to_string()).collect::<Vec<_>>().join("\n");
    std::fs::write(dir.join("log.jsonl"), rows).unwrap();
    std::fs::write(dir.join("dict.tsv"), "路霸\tlu bao\t1000\n").unwrap();
    let api = mock::MockAi::start_with_content(r#"{"accepted":[0],"domains":["软件开发"]}"#.into());
    let config = qingjian_predict::PredictConfig {
        base_url: api.url.clone(),
        api_key: Some("test-only-key".into()),
        ..Default::default()
    };
    assert!(!config.enabled);
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let (draft, domains) = super::worker::run(
        &dir.join("log.jsonl"),
        &dir.join("dict.tsv"),
        &config,
        "机器人仿真",
        &cancel,
    )
    .unwrap();
    assert_eq!(draft.words, 1);
    assert_eq!(domains, vec!["软件开发"]);
    let requests = api.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    let body = requests[0]["messages"][1]["content"].as_str().unwrap();
    assert!(!body.contains("never-send-this-app"));
    assert!(!body.contains("lubao"));
    assert!(!body.contains("陆爆"));
    assert!(body.contains("机器人仿真"));
    assert!(body.contains("录包"));
    drop(requests);
    cancel.store(true, Ordering::Release);
    assert!(
        super::worker::run(
            &dir.join("log.jsonl"),
            &dir.join("dict.tsv"),
            &config,
            "",
            &cancel
        )
        .is_err()
    );
    assert_eq!(api.requests.lock().unwrap().len(), 1);
    std::fs::remove_dir_all(dir).unwrap();
}
