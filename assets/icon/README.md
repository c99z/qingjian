# 图标

- `qingjian-mark.svg`：README 页头使用的透明竹简图标，与官网品牌图标一致。
- `logo.png`（1024×1024，带透明通道）：应用图标母版，玉绿色圆角键帽与浅色竹简，使用内置 ImageGen 生成；提示词见 `prompt.txt`。
  `apps/macos/scripts/bundle.sh` 打包时用 `sips` + `iconutil`
  生成 `Qingjian.icns`，生成物不进仓库。
- `linux/`：16 / 24 / 32 / 48 / 64 / 128 / 256 / 512 / 1024 px 的 PNG，安装到用户 `hicolor` 图标主题；
  同时登记 `qingjian` 与 GTK 应用 ID `im.qingjian.Settings` 两个名称。
- `apps/windows/tsf/resources/qingjian.ico`：同一母版生成的多尺寸 Windows 图标，供应用、设置、安装器与注册使用。
  更换母版后运行 `python3 assets/icon/generate.py`（需要 Pillow）同步 Windows 与 Linux 资源，生成物随仓库提交。
- `menu.svg`：macOS 输入法图标源文件，黑色键帽镂空四片竹简（模板图，系统只取 alpha）。`menu.pdf` 是它导出的
  22×16pt 矢量版，打包时拷成 `qingjian-menu.pdf`，Info.plist 的图标键都指向它。为什么是这个形式和尺寸见
  `docs/design/architecture.md`「Info.plist 约定」。改了 svg 重新导出：

  ```sh
  rsvg-convert -f pdf --page-width 22pt --page-height 16pt -w 22pt -h 16pt assets/icon/menu.svg -o assets/icon/menu.pdf
  ```
- `windows/mode-zh.svg` / `mode-en.svg` / `mode-caps.svg`：Windows 任务栏的中 / 英 / A 图标源文件（16×16 画布，单色）。
  `windows/render-mode-icons.sh` 用 rsvg-convert + magick 栅格化成 16 / 20 / 24 / 32 四档的 8 位 alpha 蒙版，
  写到 `apps/windows/tsf/resources/mode/`，DLL 用 `include_bytes!` 嵌入、运行时按任务栏深浅色填色（`com/mode/icon.rs`）。
  改了 svg 重跑脚本，生成物随仓库提交。
