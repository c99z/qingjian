//! 热重载的组句边界与失败保持：直接驱动 Router，不过 socket。
use super::ReloadDirs;
use crate::dispatch::{Router, RouterConfig};
use crate::protocol::admin::AdminCommand;
use qingjian_core::Engine;
use qingjian_dictionary::Dictionary;
use qingjian_platform::Config;
use qingjian_platform::protocol::{
    ClientMessage, KeyEvent, KeyModifiers, PROTOCOL_VERSION, SessionId,
};
use qingjian_platform::settings::ComponentStatus;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn dirs(root: &Path) -> ReloadDirs {
    ReloadDirs {
        resource_root: root.to_owned(),
        user_dir: None,
        bundled_dicts: None,
        input_log: None,
    }
}

fn workspace(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("qingjian-reload-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    root
}

fn router(config_path: &Path, root: &Path) -> Router {
    let config = Config::load(config_path).unwrap();
    let mut router = Router::new(
        Engine::new(Dictionary::parse("你\tni\t100\n好\thao\t90\n").unwrap()),
        RouterConfig::from(&config),
    );
    router.watch_config(&config, config_path.to_owned(), dirs(root));
    router
}

fn key(router: &mut Router, session: u64, c: char) {
    router.handle(ClientMessage::Key {
        session: SessionId(session),
        event: KeyEvent::new(c as u32, Some(c), KeyModifiers::default()),
    });
}

fn open(router: &mut Router, session: u64) {
    router.handle(ClientMessage::OpenSession {
        session: SessionId(session),
        app: None,
        protocol: PROTOCOL_VERSION,
    });
}

fn reload(router: &mut Router) -> ComponentStatus {
    let value = router
        .handle_admin(serde_json::json!({"Admin": AdminCommand::Reload}))
        .expect("reload response");
    serde_json::from_value(value["Reload"]["server"].clone()).unwrap()
}

mod behavior;
