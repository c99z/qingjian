//! 真实模型重载：配置代次变化不能丢掉仍适用的加载结果。

use std::path::Path;
use std::time::{Duration, Instant};

use qingjian_platform::Config;

use super::{reload, router, workspace};
use crate::dispatch::Router;

fn wait_loaded(router: &mut Router, context: bool) {
    let started = Instant::now();
    loop {
        router.tick();
        if router.engine.has_sentence_scorer() && router.engine.has_context_scorer() == context {
            return;
        }
        assert!(
            started.elapsed() < Duration::from_secs(15),
            "模型重载后未接上"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn model_reload_survives_configuration_generation_and_context_toggle() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let model = root.join("data/models/hanzhang-tongbian/hanzhang-tongbian-small.qjm");
    if !model.exists() {
        eprintln!("缺少真实模型，重载加载测试未验证");
        return;
    }
    let directory = workspace("context-model");
    let path = directory.join("config.toml");
    std::fs::write(&path, "[model]\nenabled=false\n").unwrap();
    let mut host = router(&path, &root);
    host.configure_local_model(Some(model), &Config::load(&path).unwrap().model);
    std::fs::write(&path, "[model]\nenabled=true\ncontext_ranking=true\n").unwrap();
    reload(&mut host);
    // 加载过程中改变无关设置：仍应保留当前模型加载任务。
    std::fs::write(
        &path,
        "[general]\npage_size=5\n[model]\nenabled=true\ncontext_ranking=true\n",
    )
    .unwrap();
    reload(&mut host);
    wait_loaded(&mut host, true);
    std::fs::write(&path, "[model]\nenabled=true\ncontext_ranking=false\n").unwrap();
    reload(&mut host);
    wait_loaded(&mut host, false);
    assert!(!host.engine.context_ranking_enabled());
    drop(host);
    std::fs::remove_dir_all(directory).unwrap();
}
