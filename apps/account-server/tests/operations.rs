//! 运维侧：健康探测、运行中一致性备份、登录锁定的边界。
use qingjian_account_server::{error::ApiError, http, probe, store::AccountStore};
use qingjian_platform::account::LoginRequest;

fn credentials(username: &str, password: &str) -> LoginRequest {
    LoginRequest {
        username: username.into(),
        password: password.into(),
    }
}
#[tokio::test(flavor = "multi_thread")]
async fn healthcheck_reaches_wildcard_bind_through_loopback() {
    let dir = tempfile::tempdir().unwrap();
    let store = AccountStore::open(&dir.path().join("accounts.sqlite3"), false).unwrap();
    let listener = tokio::net::TcpListener::bind("0.0.0.0:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let task = tokio::spawn(async move {
        axum::serve(listener, http::router(store)).await.unwrap();
    });
    let bind = format!("0.0.0.0:{port}");
    tokio::task::spawn_blocking(move || probe::health(&bind))
        .await
        .unwrap()
        .unwrap();
    task.abort();
    let _ = task.await;
    assert!(probe::health(&format!("127.0.0.1:{port}")).is_err());
    assert!(probe::health("not-an-address").is_err());
}
#[test]
fn backup_copies_accounts_while_store_is_open_and_refuses_to_overwrite() {
    let dir = tempfile::tempdir().unwrap();
    let store = AccountStore::open(&dir.path().join("db/accounts.sqlite3"), true).unwrap();
    store
        .register(credentials("alice", "test-password-long-enough"))
        .unwrap();
    let dest = dir.path().join("backup.sqlite3");
    store.backup_to(&dest).unwrap();
    assert!(matches!(store.backup_to(&dest), Err(ApiError::Invalid(_))));
    let restored = AccountStore::open(&dest, false).unwrap();
    assert!(
        restored
            .login(credentials("alice", "test-password-long-enough"))
            .is_ok()
    );
}
#[test]
fn unknown_user_and_wrong_password_are_indistinguishable() {
    let dir = tempfile::tempdir().unwrap();
    let store = AccountStore::open(&dir.path().join("accounts.sqlite3"), true).unwrap();
    store
        .register(credentials("alice", "test-password-long-enough"))
        .unwrap();
    assert!(matches!(
        store.login(credentials("nobody", "test-password-long-enough")),
        Err(ApiError::Unauthorized)
    ));
    assert!(matches!(
        store.login(credentials("alice", "wrong-password-xx")),
        Err(ApiError::Unauthorized)
    ));
    assert!(matches!(
        store.register(credentials("ALICE", "test-password-long-enough")),
        Err(ApiError::Conflict)
    ));
}
#[test]
fn concurrent_failures_all_count_and_expired_lock_restarts_counting() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("accounts.sqlite3");
    let store = AccountStore::open(&db, true).unwrap();
    store
        .register(credentials("alice", "test-password-long-enough"))
        .unwrap();
    std::thread::scope(|scope| {
        for _ in 0..5 {
            let store = store.clone();
            scope.spawn(move || store.login(credentials("alice", "wrong-password-xx")));
        }
    });
    assert!(matches!(
        store.login(credentials("alice", "test-password-long-enough")),
        Err(ApiError::RateLimited)
    ));
    let raw = rusqlite::Connection::open(&db).unwrap();
    raw.execute("UPDATE users SET locked_until=1", []).unwrap();
    assert!(
        store
            .login(credentials("alice", "wrong-password-xx"))
            .is_err()
    );
    let failures: u32 = raw
        .query_row("SELECT failures FROM users", [], |r| r.get(0))
        .unwrap();
    assert_eq!(failures, 1);
    assert!(
        store
            .login(credentials("alice", "test-password-long-enough"))
            .is_ok()
    );
}
