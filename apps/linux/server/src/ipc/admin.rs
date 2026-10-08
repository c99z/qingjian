//! 管理连接：同用户校验、版本不符明确拒绝并断开，合法请求进主线程队列与按键串行处理。
use super::{Request, connection::trusted_peer, dispatch_json};
use crate::protocol::admin::{AdminRequest, AdminResponse, LINUX_ADMIN_PROTOCOL};
use qingjian_platform::protocol::{read_message, write_message};
use serde_json::Value;
use std::os::unix::net::UnixStream;
use std::sync::mpsc::SyncSender;
use std::time::Duration;

pub(super) fn serve_admin_connection(mut stream: UnixStream, sender: SyncSender<Request>) {
    if !trusted_peer(&stream) {
        return;
    }
    let _ = stream.set_write_timeout(Some(Duration::from_secs(1)));
    while let Ok(Some(value)) = read_message::<_, Value>(&mut stream) {
        let request = match serde_json::from_value::<AdminRequest>(value) {
            Ok(request) if request.version == LINUX_ADMIN_PROTOCOL => request,
            Ok(request) => {
                reject(
                    &mut stream,
                    format!(
                        "管理协议版本不符：客户端 {}，本服务 {LINUX_ADMIN_PROTOCOL}",
                        request.version
                    ),
                );
                break;
            }
            Err(_) => {
                reject(&mut stream, "无法解析的管理请求".to_owned());
                break;
            }
        };
        let Some(response) = dispatch_json(&sender, serde_json::json!({"Admin": request.command}))
        else {
            break;
        };
        if write_message(&mut stream, &response).is_err() {
            break;
        }
    }
}

fn reject(stream: &mut UnixStream, message: String) {
    let response = serde_json::to_value(AdminResponse::Error(message)).expect("error serializes");
    let _ = write_message(stream, &response);
}
