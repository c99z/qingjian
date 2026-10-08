# 搜狗细胞词库本地导入

`import.py` 从 [搜狗词库](https://pinyin.sogou.com/dict/) 的详情页下载 `sources.json` 中的 `.scel`，调用本机 `scel2org5` 转为青简 TSV，并安装到当前用户的 `~/.local/share/qingjian/dicts/`。设置了 `XDG_DATA_HOME` 时使用相应用户目录。

```sh
sudo apt install fcitx5-chinese-addons-bin
python3 tools/sogou-dict/import.py
```

导入后重启 `qingjian-linux-server`。再次运行会保留内容相同的文件；已有文件内容不同时，需核对后显式加 `--replace`。`--cache-dir` 和 `--install-dir` 可改下载缓存与安装目录。

转换只保留纯汉字、全拼可解析且不超过 8 个音节的词条。搜狗文件的原词频均为 0，导入时统一设为 10，使专业术语可进入候选，同时避免大量旧词压过青简原有的常用词。每本词库单独安装；停用时把其文件名（不含 `.tsv`）放入青简配置 `[dictionaries].disabled`，然后重启服务。

原始 `.scel`、转换中间文件、TSV 和下载校验清单位于 `data/generated/sogou/`，该目录被 Git 忽略；用户下载的词库数据只供本机使用，不随项目发布。
