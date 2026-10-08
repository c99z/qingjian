//! 真实 HTTP 账号隔离、重启持久化、过期令牌和乐观版本同步。
use qingjian_account_server::{http, store::AccountStore};
use qingjian_platform::{
    account::{AccountSession, LoginRequest, RemoteSnapshot, UploadRequest},
    backup::{Archive, BackupPaths},
};
use reqwest::{Client, StatusCode};

fn credentials(username: &str) -> LoginRequest {
    LoginRequest {
        username: username.into(),
        password: "test-password-long-enough".into(),
    }
}
#[tokio::test]
async fn accounts_isolate_snapshots_reject_stale_upload_and_revoke_tokens() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("db/accounts.sqlite3");
    let store = AccountStore::open(&db, true).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, http::router(store)).await.unwrap();
    });
    let client = Client::new();
    assert_eq!(
        client
            .get(format!("{base}/v1/snapshot"))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let alice: AccountSession = client
        .post(format!("{base}/v1/register"))
        .json(&credentials("alice"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let bob: AccountSession = client
        .post(format!("{base}/v1/register"))
        .json(&credentials("bob"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        client
            .post(format!("{base}/v1/register"))
            .json(&credentials("alice"))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::CONFLICT
    );
    let paths = BackupPaths {
        config: dir.path().join("config.toml"),
        data: dir.path().join("user"),
        themes: dir.path().join("themes"),
    };
    std::fs::create_dir_all(&paths.data).unwrap();
    std::fs::write(paths.data.join("user.tsv"), "中文\t3\n").unwrap();
    let archive = Archive::capture(&paths).unwrap();
    let upload = || UploadRequest {
        expected_revision: 0,
        archive: archive.clone(),
    };
    let response: serde_json::Value = client
        .put(format!("{base}/v1/snapshot"))
        .bearer_auth(&alice.token)
        .json(&upload())
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(response["revision"], 1);
    assert_eq!(
        client
            .put(format!("{base}/v1/snapshot"))
            .bearer_auth(&alice.token)
            .json(&upload())
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::CONFLICT
    );
    assert_eq!(
        client
            .get(format!("{base}/v1/snapshot"))
            .bearer_auth(&bob.token)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );
    let downloaded: RemoteSnapshot = client
        .get(format!("{base}/v1/snapshot"))
        .bearer_auth(&alice.token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(downloaded.archive, archive);
    assert_eq!(downloaded.revision, 1);
    assert_eq!(
        client
            .post(format!("{base}/v1/logout"))
            .bearer_auth(&alice.token)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        client
            .get(format!("{base}/v1/snapshot"))
            .bearer_auth(&alice.token)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
    task.abort();
    let restarted = AccountStore::open(&db, false).unwrap();
    let session = restarted.login(credentials("alice")).unwrap();
    assert_eq!(session.revision, 1);
    assert_eq!(restarted.download(&session.token).unwrap().archive, archive);
    assert!(restarted.register(credentials("charlie")).is_err());
    let raw = rusqlite::Connection::open(&db).unwrap();
    let hash: String = raw
        .query_row(
            "SELECT password FROM users WHERE username='alice'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(hash.starts_with("$argon2id$"));
    assert!(!hash.contains("test-password"));
    let tokens: Vec<String> = raw
        .prepare("SELECT token_hash FROM sessions")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert!(!tokens.contains(&session.token));
    raw.execute("UPDATE sessions SET expires=0", []).unwrap();
    assert!(restarted.download(&session.token).is_err());
}
#[test]
fn failed_logins_lock_account_and_invalid_archives_are_not_stored() {
    let dir = tempfile::tempdir().unwrap();
    let store = AccountStore::open(&dir.path().join("db/accounts.sqlite3"), true).unwrap();
    let session = store.register(credentials("alice")).unwrap();
    for _ in 0..5 {
        let mut input = credentials("alice");
        input.password = "wrong".into();
        assert!(store.login(input).is_err());
    }
    assert!(matches!(
        store.login(credentials("alice")),
        Err(qingjian_account_server::error::ApiError::RateLimited)
    ));
    let paths = BackupPaths {
        config: dir.path().join("config.toml"),
        data: dir.path().join("user"),
        themes: dir.path().join("themes"),
    };
    let mut archive = Archive::capture(&paths).unwrap();
    archive.files.insert("data/../secret".into(), String::new());
    assert!(
        store
            .upload(
                &session.token,
                UploadRequest {
                    expected_revision: 0,
                    archive
                }
            )
            .is_err()
    );
    assert!(store.download(&session.token).is_err());
}
