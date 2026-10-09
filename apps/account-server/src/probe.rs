//! 健康探测：向本进程监听地址发一次 `GET /health`。监听在 `0.0.0.0` / `::` 时改连回环。
use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpStream};
use std::time::Duration;

pub fn health(bind: &str) -> std::io::Result<()> {
    let mut addr: SocketAddr = bind
        .parse()
        .map_err(|_| std::io::Error::other(format!("监听地址不是 IP:端口：{bind}")))?;
    if addr.ip().is_unspecified() {
        addr.set_ip(match addr.ip() {
            IpAddr::V4(_) => IpAddr::V4(Ipv4Addr::LOCALHOST),
            IpAddr::V6(_) => IpAddr::V6(Ipv6Addr::LOCALHOST),
        });
    }
    let timeout = Duration::from_secs(3);
    let mut stream = TcpStream::connect_timeout(&addr, timeout)?;
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;
    stream.write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")?;
    let mut response = Vec::new();
    stream.take(4096).read_to_end(&mut response)?;
    if response.starts_with(b"HTTP/1.1 200") {
        Ok(())
    } else {
        Err(std::io::Error::other("健康检查未返回 200"))
    }
}
