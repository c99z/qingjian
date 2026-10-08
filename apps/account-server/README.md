# 自建账号服务

`qingjian-account-server` 是独立的无界面服务，提供账号登录和用户备份存储。它与本机的 `qingjian-linux-server` 分开：前者部署在自己的服务器上供多台设备访问，后者负责本机输入与学习，不依赖登录。

```sh
cargo build --release -p qingjian-account-server --locked
QINGJIAN_ACCOUNT_DB=/srv/qingjian/accounts.sqlite3 \
QINGJIAN_ACCOUNT_BIND=127.0.0.1:9080 \
QINGJIAN_REGISTRATION=open \
  ./target/release/qingjian-account-server
```

数据库目录必须由运行服务的账户拥有且可写。默认数据库是当前目录 `account-data/accounts.sqlite3`，默认监听 `127.0.0.1:9080`。只有显式设置 `QINGJIAN_REGISTRATION=open` 才允许注册；建立所需账号后，可去掉此变量并重启以关闭注册。用户名 3–64 字节，接受字母、数字与 `_ - . @`；密码 12–256 字节。

远程使用时，将已有的 HTTPS 反向代理转发到此回环端口，保留 `/v1/` 路径。客户端的「账号与同步」页填写对应 HTTPS 地址；本机验证可填 `http://127.0.0.1:9080`。客户端不接受远程明文 HTTP，也不跟随携带凭据的 HTTP 重定向。反向代理应限制请求速率和正文大小（最多 96 MiB），尤其是注册与登录入口。

账号服务没有 GTK 或 Fcitx5 依赖，不需要显示环境。需要定时备份 SQLite 数据库；运行中备份请使用 SQLite 一致性备份接口，停止服务后可直接复制数据库及其 WAL 文件。不要只复制正在写入的主数据库文件。

目前同步由用户主动「上传备份」或「下载并恢复」：整份快照替换，按云端版本拒绝过期上传，不自动合并两台设备的记录。只保留每个账号的最新云端快照；本机保留上传和恢复前备份。忘记密码的自助找回、账号删除及记录级自动合并尚未提供。

协议、凭据与归档实现见 [账号服务工程记录](../../docs/notes/account-service.md)。
