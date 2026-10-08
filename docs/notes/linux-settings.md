# Linux 设置实现与验证

计划原文在本机 `/home/xiaoyu/outcome/plan/qingjian-linux-settings-development-plan.md`，P1—P3 已提交；P4 补齐实现。P5—P7 尚未完成，不能把独立设置应用标成完整版本。

## P4 验证

2026-10-08，Ubuntu 24.04 / GTK 4.14.5 / libadwaita 1.5.0 / Fcitx5，用户会话总线：

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
- P5 词库、短语、统计、备份恢复与服务协调的日志清空。
- P6 云服务、隐私和跨会话旧结果隔离。
- P7 默认安装设置应用、桌面入口、依赖检查、升级回滚与 CI。
