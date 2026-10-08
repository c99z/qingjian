# Linux 设置实现与验证

计划原文在本机 `/home/xiaoyu/outcome/plan/qingjian-linux-settings-development-plan.md`，P1—P3 已提交；P4 补齐实现。P5 数据页面、P6 无上下文云服务及 P7 安装接线已补齐，并新增自建远程账号服务。完整实机验收、后台归档性能与可选上下文仍待完成。

## P4 验证

2026-10-08，Ubuntu 24.04 / GTK 4.14.5 / libadwaita 1.5.0 / Fcitx5 5.1.23，用户会话总线：

```sh
DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus \
  cargo run -p qingjian-linux-settings --example fcitx5-roundtrip
```

验证真实 `GetConfig`、`SetConfig`、`ReloadAddonConfig` 的往返：字体改为 `Sans Bold 13`、深色预设、中文与释义的普通和高亮四种颜色、释义比例 100%、已选定制主题再改为 90%。
每次通过配置和主题的实时值读回确认；结束后恢复完整 classicui 配置及原定制主题目录。示例中任一验证失败都返回失败状态，恢复失败时保留备份目录。

这次发现释义颜色是 Optional|Color，须写 `[InputPanel/CandidateCommentColor] Value` 与对应高亮子节，主节的 `CandidateCommentColor=#...` 会被忽略。
字体写 classicui 的 `Font`，候选字号和字重由同一字体控制；释义比例设为 100 时同字号。显式主题选择关闭跟随系统，字体修改不会改变主题。

```sh
QINGJIAN_GUI_TEST=1 QINGJIAN_ADMIN_SOCKET=/tmp/qingjian-gui-no-server.sock \
QINGJIAN_GUI_CAPTURE=/tmp/qingjian-p4 DISPLAY=:1 \
XAUTHORITY=/run/user/1000/gdm/Xauthority \
DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus \
  cargo test -p qingjian-linux-settings --test gui -- --nocapture
```

真实 GTK 控件驱动验证修改仅进草稿、撤销回填、后台保存、注释保留、侧栏切页与外观探测。GTK 自身输出 1000 px 和 720 px 宽窗口的 PNG，窄窗口使用显式 800 px 断点收起侧栏。
电脑自动化截图未正确对应目标窗口，因此没有把其坐标点击计为通过，也没有据此确认真实候选面板。

单元测试覆盖外观字段映射、Fcitx5 字符串布尔、未知嵌套配置保留、主题图片复制、准备失败不破坏原主题、事务回滚、外部字段冲突、部分成功与离线草稿保留。

## 剩余验收

- 在真实 GTK / Qt / Firefox / 编辑器输入框确认候选字体、颜色与横竖排。
- 100%、150%、200% 缩放和深浅色完整页面检查。
- Ubuntu 26.04 / Wayland，其他面板接管时的适用范围。
- 大数据备份后台压缩、响应耗时与本地按键性能比较。
- 可选应用上下文（当前始终不发送）、真实多应用云候选验收。
- P7 安装包 Wayland 兼容性及完整升级回滚验收。

## P5 第一部分

统计页已显示今日 / 七天 / 累计输入量和词汇摘要；优先读服务，离线读已保存文件并显示时间。
学习与数据页有学习开关、数据目录、输入日志开关及清空按钮；清空是明确的独立操作，确认后经服务完成，不进入配置草稿。
真服务测试覆盖只有一条尚未落盘记录时清空、组句中清空、之后的新记录正常写入，以及统计不变。管理协议升至 v2。

仓库 `fmt`、非 macOS workspace 的 clippy、相关 Rust 测试、真实 GTK 检查、workspace 构建通过。
重新构建 Fcitx5 测试后发现原测试仍按单监听 socket 计数，修正管理监听的计数后 45 / 45 通过。

## 数据、云服务与账号续作

新增词库、短语、云服务、备份与恢复、账号与同步五页，共十一页。词库导入/移除、恢复、同步使用明确的独立操作；短语、开关、URL/模型与密钥均保留到应用，密钥写入私有 `.env`。Linux 云请求始终不带应用附近文字和历史输入。

管理协议升至 v3，备份含已在内存但未落盘的统计。恢复拒绝当前和挂起组句，自动创建恢复前备份；验证路径、版本、SHA256、配置与词库后，用可回滚文件事务替换并重新装配内存。云服务关闭、密钥保持本机值；主题和用户词库按完整快照恢复。归档主线程生成，后台压缩与大数据性能未完成。

新增 `apps/account-server`：独立无界面 HTTP 服务，SQLite 持久化，密码 Argon2id，会话哈希存储与过期/注销，用户间快照隔离和版本冲突检查。设置页有注册/登录、手动上传/下载恢复；远程只允许 HTTPS，账号服务地址独立于 AI URL。没有实际远程部署，也没有自动记录合并或密码找回。

2026-10-08 的实际 GTK 测试驱动保存密钥与回填，截图覆盖所有新增页及深浅色 1000/720 px 窗口；已查看云服务、备份、账号页的真实 GTK 渲染。模拟 AI HTTP 测试验证云词、Tab 整句上屏、无上下文请求、失焦与关闭云后的旧结果丢弃；真实本机账号 HTTP 测试验证用户隔离、持久化、会话注销/过期及上传冲突。备份测试覆盖组句拒绝、内存统计同步恢复、损坏校验、主题多余文件清除与回滚。

安装脚本已接设置二进制和桌面入口、依赖检查、仅输入组件选项及校验清单。安装测试验证含空格前缀的 desktop Exec、用户修改保留与卸载不删除用户数据。没有替换本机正在运行的旧安装服务；使用新增管理功能需同时升级服务和设置应用。

本次最终检查：`cargo fmt --all`、非 macOS workspace `clippy --all-targets -D warnings`、`cargo test --workspace --exclude qingjian-macos --locked`（743 passed / 0 failed / 1 ignored）和 workspace build 通过；默认测试中的 GUI 检查另以 `QINGJIAN_GUI_TEST=1` 在真实显示环境运行，1/1 通过。重新构建 Fcitx5 测试后 CTest 45/45；安装清单测试和临时隔离目录完整 `install.sh --debug --sample` / 设置程序 `--version` / 卸载均通过。本机实际用户目录没有被安装测试覆盖。
