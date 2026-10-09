# 自建账号服务

`qingjian-account-server` 是独立的无界面服务，提供账号登录和用户备份存储。它与本机的 `qingjian-linux-server` 分开：前者部署在自己的服务器上供多台设备访问，后者负责本机输入与学习，不依赖登录。

客户端对远程地址只接受 HTTPS，所以服务前面总要有一层 TLS。推荐用 Docker 部署（自带 Caddy 做 HTTPS）；已有反向代理的也可以直接跑二进制。

## Docker 部署（推荐）

`docker/` 目录里是两个容器：`account`（本服务，只在容器网络与宿主机回环上可达）和 `caddy`（对外的 HTTPS 入口）。服务器地址只出现在 `.env` 的 `QINGJIAN_SITE` 一处。

### 方式一：目标服务器上有源码

```sh
cd apps/account-server/docker
cp env.example .env          # 至少改 QINGJIAN_SITE
docker compose up -d --build
docker compose ps            # account 显示 healthy、caddy 显示 running 即可
```

### 方式二：本机打离线包，拷到目标服务器

目标服务器不需要源码、Rust 工具链或访问 crates.io：

```sh
apps/account-server/docker/package.sh            # 生成 target/account-server-docker/qingjian-account-server-<版本>-docker.tar.gz
scp target/account-server-docker/*.tar.gz user@server:
# 在目标服务器上
tar xzf qingjian-account-server-*-docker.tar.gz && cd qingjian-account-server-*-docker
docker load -i image.tar
cp env.example .env && vi .env
docker compose up -d
```

目标机是 ARM 时，打包前先 `export DOCKER_DEFAULT_PLATFORM=linux/arm64`（本机需要装 binfmt/QEMU）。拉不到 Docker Hub 时，在 `.env` 里用 `QINGJIAN_RUST_IMAGE` / `QINGJIAN_RUNTIME_IMAGE` / `QINGJIAN_CADDY_IMAGE` 换成内网镜像仓库。

### 地址与证书

| 情况 | `.env` | 客户端填写 |
|---|---|---|
| 只有 IP，或 IP 还没定 | `QINGJIAN_SITE=<IP>`，`QINGJIAN_TLS=internal` | `https://<IP>`，并信任导出的根证书 |
| 有域名，公网能访问 80/443 | `QINGJIAN_SITE=<域名>`，`QINGJIAN_TLS=<邮箱>` | `https://<域名>`，无需额外操作 |
| 443 被占用 | 另设 `QINGJIAN_HTTPS_PORT=8443` | `https://<地址>:8443` |

`QINGJIAN_SITE` 可以写多个地址，用逗号分隔（例如内网 IP 和公网 IP）。**换 IP 只需改这一行再 `docker compose up -d`**：Caddy 用同一个自建 CA 给新地址重签证书，客户端已经信任的根证书继续有效，只要在「账号与同步」页改地址并重新登录。

`internal` 模式下，把根证书导出来装到每台客户端（只需一次，CA 存在 `caddy-data` 卷里，有效期 10 年；删了这个卷才需要重装）：

```sh
docker compose exec caddy cat /data/caddy/pki/authorities/local/root.crt > qingjian-account-ca.crt
# 客户端（Debian / Ubuntu）
sudo cp qingjian-account-ca.crt /usr/local/share/ca-certificates/qingjian-account-ca.crt
sudo update-ca-certificates
```

Fedora / Arch 放到 `/etc/pki/ca-trust/source/anchors/` 或 `/etc/ca-certificates/trust-source/anchors/` 后运行 `sudo update-ca-trust`。根证书私钥同在 `caddy-data` 卷里，卷的备份要和数据库一样妥善保管。

### 注册与日常运维

- `QINGJIAN_REGISTRATION=open` 时允许注册。建好需要的账号后改成 `closed`，再 `docker compose up -d` 即关闭。
- 日志：`docker compose logs -f account`；`RUST_LOG=debug` 看更多。
- 备份（服务不用停）：

  ```sh
  docker compose exec account qingjian-account-server backup /data/backup-$(date +%F).sqlite3
  docker compose cp account:/data/backup-$(date +%F).sqlite3 .
  docker compose exec account rm /data/backup-$(date +%F).sqlite3
  ```

- 恢复：`docker compose stop account`，把备份文件拷成卷里的 `/data/accounts.sqlite3`（同时删掉旧的 `accounts.sqlite3-wal` / `-shm`），属主保持 uid 10001，再 `docker compose start account`。
- 升级：方式一 `git pull && docker compose up -d --build`；方式二 `docker load` 新镜像、改 `.env` 里的 `QINGJIAN_ACCOUNT_IMAGE` 后 `docker compose up -d`。数据在 `account-data` 卷里，重建容器不丢。
- 防火墙只需放行 HTTPS 端口；`internal` 模式下 80 端口只做跳转，可以不放行。账号服务的 9080 只发布在宿主机 `127.0.0.1`。

## 直接运行二进制

```sh
cargo build --release -p qingjian-account-server --locked
QINGJIAN_ACCOUNT_DB=/srv/qingjian/accounts.sqlite3 \
QINGJIAN_ACCOUNT_BIND=127.0.0.1:9080 \
QINGJIAN_REGISTRATION=open \
  ./target/release/qingjian-account-server
```

| 变量 | 默认 | 说明 |
|---|---|---|
| `QINGJIAN_ACCOUNT_BIND` | `127.0.0.1:9080` | 监听地址；镜像里是 `0.0.0.0:9080` |
| `QINGJIAN_ACCOUNT_DB` | `account-data/accounts.sqlite3` | 数据库路径；目录须由运行服务的账户拥有且可写 |
| `QINGJIAN_REGISTRATION` | 关闭 | 只有值为 `open` 才允许注册 |
| `RUST_LOG` | `info` | 日志级别 |

子命令：`healthcheck` 探测本机 `/health`（容器健康检查用），`backup <目标文件>` 运行中做一致性备份（目标已存在时拒绝），`--version`。服务收到 SIGTERM / Ctrl-C 后处理完进行中的请求再退出。

远程使用时，将已有的 HTTPS 反向代理转发到此回环端口，保留 `/v1/` 路径，正文上限至少 96 MiB + 1 KiB。客户端的「账号与同步」页填写对应 HTTPS 地址；本机验证可填 `http://127.0.0.1:9080`。客户端不接受远程明文 HTTP，也不跟随携带凭据的 HTTP 重定向。

## 限制

用户名 3–64 字节，接受字母、数字与 `_ - . @`；密码 12–256 字节。连续 5 次错误密码锁定该账号 1 分钟；同时最多处理 8 个请求，超出返回 503。服务本身不按 IP 限速，暴露到公网时建议在防火墙或反向代理上再加一层。

不要只复制正在写入的主数据库文件：运行中用 `backup` 子命令，停服后可以连同 WAL 文件一起复制。

目前同步由用户主动「上传备份」或「下载并恢复」：整份快照替换，按云端版本拒绝过期上传，不自动合并两台设备的记录。只保留每个账号的最新云端快照；本机保留上传和恢复前备份。忘记密码的自助找回、账号删除及记录级自动合并尚未提供。

协议、凭据与归档实现见 [账号服务工程记录](../../docs/notes/account-service.md)。
