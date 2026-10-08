//! 本机 OpenAI 兼容模拟服务，记录正文并返回固定结果，不使用外网密钥。
use std::io::{Read, Write};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;
pub struct MockAi {
    pub url: String,

    pub requests: Arc<Mutex<Vec<serde_json::Value>>>,

    stop: Arc<AtomicBool>,

    thread: Option<std::thread::JoinHandle<()>>,
}
impl MockAi {
    pub fn start() -> Self {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let captured = requests.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = stop.clone();
        let thread = std::thread::spawn(move || {
            while !worker_stop.load(Ordering::Relaxed) {
                let (mut stream, _) = match listener.accept() {
                    Ok(connection) => connection,
                    Err(_) => {
                        std::thread::sleep(Duration::from_millis(5));
                        continue;
                    }
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut bytes = Vec::new();
                let mut chunk = [0u8; 4096];
                let body = loop {
                    let count = match stream.read(&mut chunk) {
                        Ok(0) | Err(_) => break None,
                        Ok(n) => n,
                    };
                    bytes.extend_from_slice(&chunk[..count]);
                    if let Some(end) = bytes.windows(4).position(|b| b == b"\r\n\r\n") {
                        let header = String::from_utf8_lossy(&bytes[..end]).to_ascii_lowercase();
                        let size: usize = header
                            .lines()
                            .find_map(|line| {
                                line.strip_prefix("content-length:")
                                    .and_then(|n| n.trim().parse().ok())
                            })
                            .unwrap();
                        if bytes.len() >= end + 4 + size {
                            break Some(
                                serde_json::from_slice::<serde_json::Value>(
                                    &bytes[end + 4..end + 4 + size],
                                )
                                .unwrap(),
                            );
                        }
                    }
                };
                if let Some(body) = body {
                    captured.lock().unwrap().push(body);
                    std::thread::sleep(Duration::from_millis(100));
                    let content = serde_json::json!({"words":[{"text":"拟好","pinyin":"ni hao"}],"sentence":"你好，世界。"}).to_string();
                    let response = serde_json::json!({"id":"mock", "object":"chat.completion", "created":0, "model":"mock", "choices":[{"index":0,"message":{"role":"assistant","content":content},"finish_reason":"stop"}]}).to_string();
                    let _ = write!(
                        stream,
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        response.len(),
                        response
                    );
                }
            }
        });
        Self {
            url,
            requests,
            stop,
            thread: Some(thread),
        }
    }
}
impl Drop for MockAi {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.thread.take().unwrap().join().unwrap();
    }
}
