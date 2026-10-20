//! Linux 按下 / 释放与 Poll 闭环：简拼命中的首选不被迟到的模型生成替换。

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use qingjian_core::Engine;
use qingjian_core::sentence::SentenceScorer;
use qingjian_dictionary::Dictionary;
use qingjian_linux_server::{Router, RouterConfig};
use qingjian_platform::protocol::{
    ClientMessage, KeyEvent, KeyModifiers, PROTOCOL_VERSION, SessionId,
};
use serde_json::{Value, json};

struct Generates(Arc<AtomicUsize>);

impl SentenceScorer for Generates {
    fn score(&self, _: &str, _: &str, texts: &[&str]) -> Vec<f64> {
        vec![-1.0; texts.len()]
    }

    fn generate(&self, _: &str, _: usize, _: usize) -> Vec<String> {
        self.0.fetch_add(1, Ordering::SeqCst);
        vec!["候选C".into()]
    }
}

fn key(router: &mut Router, character: char, release: bool) -> Value {
    router
        .handle_linux(json!({"LinuxEvent":{"session":1,"event":{"Key":{
            "event":KeyEvent::new(character as u32, Some(character), KeyModifiers::default()),
            "release":release
        }}}}))
        .unwrap()["KeyResult"]
        .clone()
}

#[test]
fn polling_keeps_the_abbreviated_word_and_space_commits_what_was_shown() {
    let calls = Arc::new(AtomicUsize::new(0));
    let engine = Engine::new(
        Dictionary::parse("候选词\thou xuan ci\t1000\n候选\thou xuan\t1000\n").unwrap(),
    )
    .with_async_sentence_scorer(Box::new(Generates(Arc::clone(&calls))), None, None, None);
    let mut router = Router::new(engine, RouterConfig::default());
    router.handle(ClientMessage::OpenSession {
        session: SessionId(1),
        app: None,
        protocol: PROTOCOL_VERSION,
    });
    router.handle_linux(json!({"LinuxEvent":{"session":1,"event":{"Capabilities":{
        "sensitive":false,"password":false,"disabled":false
    }}}}));
    let mut initial = Value::Null;
    for c in "houxuanc".chars() {
        initial = key(&mut router, c, false)["frame"].clone();
        key(&mut router, c, true);
    }
    assert_eq!(initial["candidates"]["items"][0]["text"], "候选词");
    let started = Instant::now();
    while started.elapsed() < Duration::from_millis(300) {
        std::thread::sleep(Duration::from_millis(20));
        router.tick();
        let update = router.handle_linux(json!({"Poll":{"session":1}})).unwrap();
        assert_eq!(
            update["Update"]["frame"]["candidates"],
            initial["candidates"]
        );
    }
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(key(&mut router, ' ', false)["commit"], "候选词");
}
