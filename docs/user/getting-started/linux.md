---
title: Linux
order: 6
description: 在 Linux 上安装青简，使用 Fcitx5 默认候选面板。
---

青简 Linux 提供预编译包与源码安装，使用 Fcitx5 默认候选面板。已验证 Ubuntu 26.04 的 GNOME 桌面（Wayland）上 Fcitx5 5.1.19 的 GTK4、Qt6 应用与 Firefox；
其他桌面、其他发行版和旧版应用尚未完成验证。当前源码支持本地候选、学习、本地整句模型重排及可选云联想，并提供独立设置窗口。服务仍需手动启动；较早发布的预编译包不包含新增设置功能。

## 预编译包

预编译包在 Ubuntu 26.04（x86_64）上构建，插件与系统的 Fcitx5 版本绑定；其他发行版请用下一节的源码安装。
先装好 Fcitx5 与 GTK / Qt 输入支持：

```sh
sudo apt install python3 fcitx5 fcitx5-frontend-gtk3 fcitx5-frontend-gtk4 fcitx5-frontend-qt6 fcitx5-config-qt
```

从[下载页](https://qingjian.app/download)下载 `qingjian-<版本>-linux-x86_64.tar.gz`，解压后在解出的目录里执行：

```sh
./install.sh

# 手动启动，保持此终端运行
~/.local/bin/qingjian-linux-server
```

之后的添加输入法、首次输入与源码安装相同，见下一节末尾。更新时下载新包再执行一次 `./install.sh`；卸载用包里的 `./uninstall.sh`。

## 源码安装和首次输入

先安装 Rust 1.96、CMake、C++20 编译器、pkg-config、Fcitx5、其 GTK / Qt 输入支持，以及 OpenSSL、Fcitx5 Core/Config/Utils 与 nlohmann-json 的开发包。
Debian / Ubuntu 上是：

```sh
sudo apt install cmake g++ pkg-config python3 libssl-dev \
  libfcitx5core-dev libfcitx5utils-dev libfcitx5config-dev nlohmann-json3-dev \
  fcitx5 fcitx5-frontend-gtk3 fcitx5-frontend-gtk4 fcitx5-frontend-qt6 fcitx5-config-qt \
  libgtk-4-dev libadwaita-1-dev
```

缺了哪一项，安装脚本开头会直接列出来。
按发行版指引启用 Fcitx5；X11 应用通常需要 `GTK_IM_MODULE=fcitx`、`QT_IM_MODULE=fcitx`、`XMODIFIERS=@im=fcitx`，环境变化后重新登录。

在源码目录执行：

```sh
# 下载并校验正式词库与本地整句模型
tools/release/data-fetch.sh
apps/linux/scripts/install.sh

# 手动启动，保持此终端运行
~/.local/bin/qingjian-linux-server
```

也可用 `apps/linux/scripts/install.sh --sample --debug` 快速体验少量样例词（例如「你好」），不下载正式词库。
默认安装到 `~/.local`；`--prefix /绝对用户目录` 可更改安装位置。请用相同用户安装、运行，不要使用 sudo。

重启 Fcitx5，打开 Fcitx5 配置工具，取消「仅显示当前语言」，添加「青简」。切换到青简后输入 `nihao`，空格选中「你好」。
单击 `Shift` 切换中英；按键规则见 [按键与快捷键](keys.md#Linux（Fcitx5）)。关闭手动启动的终端会结束青简服务；下次登录后需再次启动。

## 设置窗口

新版完整安装后，可以从应用菜单打开「青简设置」，也可以在 Fcitx5 托盘图标的菜单里点「青简设置」，或在 Fcitx5 配置工具的输入法列表里选中「青简」后点「配置」。还可以运行：

```sh
~/.local/bin/qingjian-linux-settings
```

开发时也可以在源码目录构建并打开，需要 GTK4（至少 4.14）与 libadwaita（至少 1.5）：

```sh
sudo apt install libgtk-4-dev libadwaita-1-dev
cargo build -p qingjian-linux-settings
./target/debug/qingjian-linux-settings
```

修改后点击「应用」统一保存，窗口会分别提示青简设置和外观的生效结果。关闭窗口不影响输入法。
外观设置适用于 Fcitx5 经典面板，会影响共用该面板的其他输入法；详情见 [偏好设置](../settings/preferences.md#Linux-设置窗口（开发版）)。

## 配置、隐私和数据

首次运行生成 `~/.config/qingjian/config.toml`。源码开发版服务会自动读取保存后的修改；正在输入时，相关设置等所有输入窗口本轮输入结束后生效。较早安装包仍需重启青简服务。
`[general] preedit` 可设为 `both`（行内和候选窗口）、`inline`（只在行内）、`window`（只在候选窗口）；应用不支持行内显示时使用候选窗口。
每页候选数、翻页键、学习、日志和辅助语言使用同一配置文件。`learning_language = "off"` 关闭中文候选的辅助语言释义与生词标记。系统面板外观由 Fcitx5 设置控制，也可通过源码开发版的「候选与外观」页修改。
当前预编译包随附含章·通变与含章·知微：优先用通变处理拼音整句与纠错，没有通变时回退到知微。自己的 `.qjm` 也可放入 `~/.local/share/qingjian/models/` 下对应的模型目录。`[model] enabled = false` 可关闭本地模型，见 [本地整句模型](../input/local-model.md)。

`[general] shift_letter = "compose"` 让 Shift 大写字母参与中文组句，默认 `"passthrough"` 保持临时英文输入。
`scheme = "zhuyin"` 启用大千注音；双拼下 `Shift + V` / `Shift + U` 可进入表达式 / 码点输入。
数字没有对应候选时继续输入，英文直输内容以空格结束时保留空格；英文候选开启后可用数字、翻页键、空格或 Tab 选词。
具体规则见 [按键与快捷键](keys.md#Linux（Fcitx5）)。

Fcitx5 识别为敏感输入时，可以组句但不会保存输入文本或学习；识别为密码框或禁用输入法的输入框时直接交还应用。
保护依赖应用和其输入支持正确传递标记；本机已验证 Qt6 的敏感标记，GTK4 的动态 PRIVATE 提示尚未传递为敏感输入标记。
普通输入的学习与日志遵循配置。学习数据保存在 `~/.local/share/qingjian`，运行日志保存在 `~/.local/state/qingjian/logs`。
设置了 `XDG_CONFIG_HOME`、`XDG_DATA_HOME`、`XDG_STATE_HOME` 时分别使用对应目录下的 `qingjian`。

## 更新和卸载

结束手动启动的青简服务，重复执行安装命令，然后重启 Fcitx5 和青简。安装会检查文件归属；目标文件被手动修改时会提示保留，请先备份处理。

```sh
apps/linux/scripts/uninstall.sh
# 自定义安装位置时：
apps/linux/scripts/uninstall.sh --prefix /安装时的绝对目录
```

卸载后手动结束青简服务并重启 Fcitx5。卸载保留配置、个人词库、学习数据、已修改的安装文件和其他输入法。

完整源码安装默认包含 GTK 设置程序和桌面菜单入口，需要 `libgtk-4-dev` 与 `libadwaita-1-dev`；只安装输入组件可加 `--input-only`。设置与本机服务需使用同一新版，旧服务缺少管理接口时，统计、备份恢复和云配置生效会提示不可用。请完成未上屏输入后手动退出旧服务，再启动新安装的 `qingjian-linux-server`。

设置的「账号与同步」页支持连接自建的无界面账号服务，注册、登录并在设备间上传或恢复数据，详见 [账号、备份与同步](../settings/account-and-backup.md)。账号服务单独部署，不随输入法安装自动启动。
