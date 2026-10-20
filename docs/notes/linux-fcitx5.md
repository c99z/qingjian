# Linux Fcitx5 默认面板

第一阶段保留 Rust Server、薄插件和用户目录安装。输入模式、组句、选词、翻页、隐私和失焦提交均在 Server；
插件负责 Fcitx 事件与默认预编辑 / 候选 API。长期自绘方向不变，后续由 Server 渲染并管理 X11 窗口，或向 GNOME 扩展发送位图；本阶段不带相关源码、补丁、渲染 FFI、服务管理或 Debian 包。

## 构建与测试

需要 Rust 1.96、CMake 3.16、C++20 编译器、pkg-config，以及 OpenSSL（`libssl-dev`，云服务依赖的 TLS 绑定要它）、Fcitx5 Core/Config/Utils 和 nlohmann-json 的开发包；`install.sh` 开头会检查并列出缺的。
构建最低 API 为 Fcitx5 5.1.8，CI 使用 Ubuntu 26.04 的 Fcitx5 5.1.19；Ubuntu 24.04 自带的 5.1.7 不满足要求。
桌面运行已验证的范围见下文，构建版本门槛不代表全部桌面已验收。

```sh
cargo test -p qingjian-linux-server --locked
cargo fmt --all --check
cargo clippy --workspace --exclude qingjian-macos --all-targets --locked -- -D warnings
cargo test --workspace --exclude qingjian-macos --locked
cargo build -p qingjian-linux-server --locked
cmake -S apps/linux/fcitx5 -B target/fcitx5-stage1 -DCMAKE_BUILD_TYPE=Debug -DBUILD_TESTING=ON
cmake --build target/fcitx5-stage1 --parallel 2
env -u DBUS_SESSION_BUS_ADDRESS ctest --test-dir target/fcitx5-stage1 --output-on-failure
python3 apps/linux/tests/installation.py
```

CTest 的 `real-server` 实际运行 Rust Server 并通过 InputContext 输入与点击，默认二进制位置 `target/debug/qingjian-linux-server`。
自定义 Cargo target 时传 `-DQINGJIAN_SERVER_EXECUTABLE=/绝对路径/qingjian-linux-server`。
测试实例关闭所有 Fcitx addon，不依赖桌面总线；插件初始化本身也不连接 D-Bus。真实桌面测试才需要 `fcitx5-modules`、GTK / Qt 输入模块及 `dbus-run-session`。

## 协议与状态

同用户 Unix socket，长度前缀为 4 字节小端，单消息上限 16 MiB；每次插件收发共用 200 ms 截止时间。
插件只持有一条连接和一个 I/O watcher，每个 InputContext 分配独立、递增的 session ID。
每个会话先 `OpenSession` 校验共享协议版本（6），再协商 `LinuxHello` v3，携带 session/generation/context，返回会话编号和既有预编辑设置。
首次能力确认前不接受输入；握手与能力状态逐会话保存。同一连接支持超过 64 个上下文，64 条连接的资源保护不限制单连接会话数。
Linux 的首个回包仍是含 `linux_ui` 的 `Update`，不额外发送 Windows DLL 使用的 `SessionOpened`；旧版插件的协议 5 连接会被拒绝，升级时需同时更新 Server 与插件。
旧版本或损坏响应关闭连接、清空显示并放行当前输入；重连创建全新输入状态，旧提交不会重放。
显示 API 同步触发能力变化、失焦或 Reset 时，插件在最终上屏前撤销旧生命周期响应，但保持已接受按键的 Consumed 结果；纯显示失败仍交付有效提交。

同一上下文停用后保留会话和中英模式；Reset 只丢弃输入。内部中英切换读 `[shortcut] switch_mode`，
单击 Shift / Ctrl 在释放时触发，Ctrl + Alt + Space 在按下时触发；组合键、焦点变化、停用与配置重载撤销待确认的单击。
Fcitx5 的输入法切换独立保留；全局 Shift 已绑定时可将青简改为 Ctrl + Alt + Space。
Password / Disable 关闭目标会话，恢复普通输入后分配新编号。
销毁上下文时撤销安全引用并排队 CloseSession，由事件循环延迟发送，不阻塞析构，也不关闭其他会话。
共享连接故障先让全部会话失效，再清理各自显示；清理回调期间不重连。

LinuxEvent 转发能力、按下 / 释放、焦点、停用原因、客户端预编辑事实、带帧身份的候选点击和翻页。
空能力是普通输入；Sensitive 可组句但不学习、不记输入文本；Password / Disable 优先禁用并丢弃输入。
能力先由 Server 确认，再清理可能同步重入的预编辑；Reset / 焦点变化不会丢掉能力通知，清理中重入的新按键已受新隐私状态约束。
能力变化清理组句、暂存透传、学习链与补全；能力即将改变时的 deactivate 原因也触发清理，即使框架此刻仍返回旧能力。
FocusOut 的行内预编辑由框架或声明 ClientUnfocusCommit 的客户端提交，Server 不重复返回它；仅窗口预编辑才返回原样组句。

每次候选响应绑定 generation/context/revision，revision 在整个 Server 内递增；过期或跨会话点击返回 Ignored，不改变当前会话的展示状态。
默认面板只展示第一条释义，完成面板更新后报告当前页对应的 `(候选槽位, 0)`。
`[general] layout` 传为 Fcitx5 的 `CandidateLayoutHint`。竖排保留正文 / comment 并列；横排有释义时，
`Word` 将第一条释义作为带 `DontCommit` 的第二行附在展示正文中，comment 留空，序号同步保留第二行，
整页候选等高、缺释义处留白。整页无释义时维持单行。数字键 / 点击仍使用原始槽位，提交正文不包含换行与释义。
经典面板的正文与 comment 固定并排，多行正文共用候选字号与颜色，所以独立 comment 样式仅用于竖排；设置预览遵循相同规则。
隐藏、失焦、私密、无候选与过期回报不产生有效展示记录；Server 只凭已生成帧不记展示。

组句期间插件每 20 ms 发一次 `Poll`（与 Server 等模型结果的轮询间隔相同），Server 先 tick 再回当前帧。
帧与该会话上次发出的相同时沿用展示身份（revision 不变），插件不重画也不重复回报；本地整句模型换了顺序才推进 revision，插件按版本号推进重画并回报。
帧为空、Reset、失焦、停用、断线时插件停表；协议版本不变。

## 管理 socket 与配置热重载

管理 socket 与按键 socket 分开：同目录的 `qingjian-admin.sock`（`QINGJIAN_ADMIN_SOCKET` 可覆盖），同样的 0600 权限、父目录检查、陈旧清理、长度前缀 JSON 与同 uid 校验。
协议独立版本化（`LINUX_ADMIN_PROTOCOL = 2`），版本不符或无法解析的请求回 `Error` 并断开；请求进主线程同一队列，与按键处理串行。
当前命令：`status`（版本、配置代次与挂起状态、上次错误、模型状态、云服务 disabled 占位）、`reload`（立即读盘应用，返回 `ComponentStatus`）、`stats`（今日 / 近七天 / 累计输入量与词汇统计），`clear_input_log`（关闭旧日志缓冲后截断，按已应用的开关重开写入端，返回 `Maintenance(ComponentStatus)`）。

热重载在 `dispatch/reload`：一秒节拍轮询 `config.toml` 的 mtime（原子替换后按路径重读即可），解析失败保持上一次可用配置并把错误留在 `status` 里。
学习开关立即生效；输入方案、快捷键、候选数量、释义语言、附加词库、模型与输入日志开关等所有会话（含挂起会话）的组句结束后才应用——组句中保存设置不丢正在输入的拼音与候选。
每次成功应用递增配置代次；模型后台加载带代次，旧代次的结果不覆盖新设置。主词库与云服务不在重载范围内。
释义表热换走 mmap `.qj` 或随包 TSV，加载失败保持旧语言；展示设置刷新到 `RouterConfig`，新起的帧立即生效。

中文组句的 `[shortcut] translation_space`（缺省 `shift`）配合空格调用 Core `commit_translation`，上屏高亮候选的第一条释义。
缺释义时保留输入；英文组句、直输段与表达式 / 问字模式不拦截。普通空格仍选原候选，Shift 松开不切换中英。

## 本地整句模型

Server 启动时按 `[model] enabled`（缺省开）在后台线程加载模型并预热，`find_model` 优先找用户目录 `~/.local/share/qingjian/models/hanzhang-tongbian/` 与随包的 `resources/data/models/hanzhang-tongbian/`，然后回退知微；旧用户目录仍可读取。开关随配置热重载生效（见下节），无需重启服务。
节拍与 Windows Server 的 `dispatch/rescore` 相同：缓冲变化后起 80 ms 防抖，到点把整句路径送去后台打分，每 20 ms 收一次，最长等 2 s；
结果到了只在用户还看着第一页、没动过高亮时重建候选布局，下一次 `Poll` 回的帧就是新顺序。主循环按 `Router::next_tick` 的绝对到点时间等消息，空闲时一秒一次落盘学习。
模型在后台接上时用户正在组句，Server 补查一次攒下整句路径再起防抖，这一轮不错过重排。前文用本会话最近上屏的字，首版不读应用光标前文。

## 路径和排错

用户安装与手动启动见 [Linux 用户说明](../user/getting-started/linux.md)。安装只登记实际绝对插件库路径，不修改系统 Fcitx5 搜索规则。
`QINGJIAN_SOCKET` 可指定绝对 socket 路径；缺省为 `$XDG_RUNTIME_DIR/qingjian.sock`，无 runtime 时用 `/tmp/qingjian-<uid>/qingjian.sock`。
父目录须归当前用户且不可被其他用户写入；socket 权限为 0600。`QINGJIAN_RESOURCES` 覆盖资源根，`QINGJIAN_DICT` 可指定测试词库。
默认数据从可执行文件旁 `../share/qingjian/resources` 找，支持源码开发目录回退。

未出候选时先确认手动 Server 在运行，再查看 `$XDG_STATE_HOME/qingjian/logs/server.*.log`（缺省 `~/.local/state`）。
输入日志由 `[general] input_log` 控制；用户配置和学习文件始终保存在 XDG 用户目录，卸载不删除。

## 验证环境

- Ubuntu 26.04，未打补丁的 Fcitx5 5.1.19-1 / Core7 / modules，GTK4 4.22.4、Qt6 6.10.2。
- 隔离 X11（Xvfb）、私有 D-Bus：GTK4、Qt6。
- 真实桌面（2026-09-20）：GNOME 50 原生 Wayland，文本编辑器（GTK4）、FeatherPad（Qt6）、Firefox（snap）。Ubuntu 26.04 的 GNOME 已没有 X11 登录项。
  默认面板的外观与位置由 Fcitx5 决定：GTK4 应用里是输入模块自己画的黑白面板，Firefox 里是 Fcitx5 的橙色面板且与光标同一行，Fcitx5 自带拼音表现相同。
- 真实桌面（2026-09-22，本地整句模型）：CachyOS，niri（原生 Wayland），Fcitx5 5.1.22，Microsoft Edge（Chromium，`--enable-wayland-ime`）与 Konsole（Qt6）：停顿后整句候选按模型换序。
- 横排两行（2026-10-12）：Ubuntu 24.04.3，Fcitx5 5.1.23、GTK4 4.14.5 / libadwaita 1.5.0；Xephyr 隔离 X11 与私有 D-Bus，实际经典候选面板截图确认中文 / 释义上下对应、缺释义留白，数字 1 上屏「你好」且不带展示换行。真实 GTK 设置窗口验证横竖排草稿、保存与预览。
- 未验证：KDE 与其他桌面、其他发行版、GTK3 / Qt5 应用、真实 X11 桌面会话。

设置应用统计页服务离线时读 `usage.tsv` / `user-vocab.tsv` 并标明更新时间。清空日志只由服务操作既定日志路径，不接收外部文件参数。
插件集成夹具中的 socket 总数为三：按键监听、管理监听、共享插件连接；多个上下文仍复用同一条插件连接。
