# 自建账号服务与数据同步

2026-10-08 增加 `apps/account-server`（`qingjian-account-server`，0.1.5-dev），独立于本机输入服务。不加载 Engine，不处理按键。Axum HTTP 层把口令哈希和 SQLite 操作交给阻塞工作线程；最多同时处理 8 个请求，超出返回 503（`/health` 不计入，免得忙时被编排判成故障）。所有账号、会话、快照在同一 SQLite 数据库中，开启 WAL，参数化查询，以账号 ID 隔离快照。

## HTTP v1

| 接口 | 正文 / 响应 |
|---|---|
| `GET /health` | `ok` |
| `POST /v1/register` | `{username,password}` → AccountSession；默认关闭，`QINGJIAN_REGISTRATION=open` 才开放 |
| `POST /v1/login` | `{username,password}` → AccountSession |
| `POST /v1/logout` | Bearer token，撤销会话，204 |
| `GET /v1/snapshot` | Bearer token → `{revision,archive}`；尚无快照 404 |
| `PUT /v1/snapshot` | Bearer token，`{expected_revision,archive}` → `{revision}`；版本不符 409 |

共享类型在 `qingjian_platform::account`。没有把账号凭据混进 AI 请求。用户名转为小写；密码用随机盐 Argon2id PHC 存储。连续五次错误密码锁定账号一分钟；失败计数在库锁内读改写，并发的错误登录不会少记。Argon2 校验在数据库锁外做，一次登录不挡住其他请求；不存在的用户名也对一个随机哈希校验一次，响应时间不暴露账号是否存在。会话令牌为两个 CSPRNG UUID v4 的随机串，数据库只存 SHA-256，30 天有效，注销删除。客户端本机 `~/.config/qingjian/account-session.json` 为 0600，保存 URL、用户名、令牌及已确认的云端版本；密码不落盘。错误响应不暴露 SQL、口令或令牌。

默认监听回环地址，TLS 由自建服务器的反向代理终止。存储错误记 `tracing::error!`（含路径与系统错误），响应里只给笼统文案。SIGTERM / Ctrl-C 优雅退出：容器里服务是 PID 1，内核不装默认 SIGTERM 处理，不接住的话 `docker stop` 要等到超时 SIGKILL。GUI 只允许 HTTPS，回环地址可用 HTTP；禁用重定向，拒绝 URL 内用户名、密码、查询与片段。配置页修改账号地址后，旧会话只能在原地址使用，必须显式重新登录。上传与恢复都要求先处理设置草稿。

## Docker 部署

`apps/account-server/docker/`：多阶段 `Dockerfile`（构建上下文是仓库根，排除规则在 `Dockerfile.dockerignore`，必须排除 `.env` 以免 AI 密钥进构建层），`compose.yaml` 起 `account` 与 `caddy` 两个容器，`package.sh` 打含镜像的离线包。

- 地址只在 `.env` 的 `QINGJIAN_SITE`，Caddyfile 用 `{$QINGJIAN_SITE}` 展开，可写逗号分隔的多个地址。`QINGJIAN_TLS=internal` 用 Caddy 自建 CA（根证书 10 年，存 `caddy-data` 卷），换 IP 后重签的叶证书仍由同一根签发，客户端不用重装信任；填邮箱走 ACME。
- 没有做「只靠客户端连进来的地址自动签证书」（`tls internal { on_demand }`）：客户端用 IP 直连不发 SNI，Caddy 退回用连接的本地地址，在 Docker 端口映射后是容器网段的 172.x，签出的证书对不上。
- 客户端没有放宽「远程只接受 HTTPS」去迁就内网 IP：内网明文同样会把口令与令牌暴露给同网段。自建 CA 根证书装进系统信任库即可，客户端的 native-tls（Linux 上是 OpenSSL）按 IP SAN 校验。
- `account` 容器以 uid 10001、只读根文件系统、`cap_drop: ALL` 运行，9080 只发布到宿主机 `127.0.0.1`；镜像里没有 curl，健康检查用二进制自带的 `healthcheck` 子命令（`0.0.0.0` 监听时改连回环）。
- 构建阶段设 `RUSTUP_TOOLCHAIN=${RUST_VERSION}`，不让 `rust-toolchain.toml` 触发 rustup 联网装 clippy / rustfmt。运行时要 `libssl3t64`：`qingjian-platform` 经 `qingjian-predict` 依赖 reqwest native-tls，二进制链接了 libssl。
- 运行中备份用 `backup <目标>` 子命令（`VACUUM INTO`，读的是一个事务快照，不必停服）。

## 归档与本机协调

共享 `qingjian_platform::backup::Archive` 为 v1 JSON `.qjb`：逻辑路径、Base64 正文、逐文件 SHA-256 校验和与生成时间。不含 `.env`、账号会话、输入日志、运行日志或模型；配置中去掉凭据字段，强制关闭云联想。数据上限 64 MiB，归档上限 96 MiB，最多 10000 文件。目前不压缩、不加密；传输使用 HTTPS，落盘 0600。

本机管理协议升至 v3：`backup` 返回服务生成的固定 `data/backups/<uuid>.qjb` 路径；`restore` 只接受传输 UUID，从 `data/transfers/<uuid>.qjb` 读取，不接受任意文件路径。设置与本机服务需一起更新，按键及 Linux UI 协议不变。

服务先通过新增 `flush_learning_checked` 确认词频、用户词、个人 n-gram、统计、词汇与个人释义已落盘。此管理命令与按键串行执行，快照归档在命令期间完成；大型备份会有维护耗时，尚未拆成后台压缩任务。运行中的用户数据不能由 GUI 直接覆盖。

恢复验证版本、路径、校验和、配置、UTF-8 与用户词库可加载性。只恢复白名单中的配置、学习文件、`dicts/` 和 `qingjian-custom` 主题文件，不改 Fcitx5 的全局经典面板选择或字体配置。任何聚焦或挂起会话尚在组句时立即拒绝，要求完成输入后重试；不静默清掉拼音。恢复前自动备份；逐文件保留回滚正文，写入与内存重载成功才提交。失败显式回滚；没有持久维护锁，连接关闭不会留下锁。恢复后重新装配学习、统计、词汇、个人释义与附加词库，保留本机 AI 密钥。归档主题文件的生效随 Fcitx5 下一次主题重载。

跨设备同步先上传或下载完整归档。下载文件在本机 `backups/from-cloud-<uuid>.qjb` 保留，再走同一本机恢复流程。SQLite 单事务检查 expected_revision，其他设备已更新时拒绝覆盖。暂不自动合并、增量同步、提供账号密码找回或对外部署实例。服务本身不按 IP 限速，暴露到公网时在防火墙或反向代理上加。

## 验证

- `cargo test -p qingjian-account-server`：真实 HTTP 注册登录、账号隔离、409 版本冲突、注销、过期令牌、重启持久化、密码错误锁定（含并发计数与锁定过期重新计数）、不存在的用户与错误密码同为 401、无效归档拒绝、健康探测、运行中备份。
- Docker：`docker compose up -d --build` 后 `docker compose ps` 两个容器 healthy / running；用导出的根证书 `curl --cacert qingjian-account-ca.crt https://<IP>:<端口>/health` 得 `ok`。
- `cargo test -p qingjian-platform backup`：凭据排除、未知版本、越界路径、符号链接、校验和损坏、恢复提交与回滚。
- Linux 真服务管理测试：未落盘统计进入备份；组句中拒绝恢复且继续正常输入；恢复后内存与下一条记录正确。
