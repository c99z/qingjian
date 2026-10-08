#!/usr/bin/env python3
# //! 从搜狗词库详情页下载 .scel，转换并安装为当前用户的青简附加词库。

from __future__ import annotations

import argparse
import hashlib
import html
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
from urllib.parse import quote, urljoin, urlparse
from urllib.request import Request, urlopen


ROOT = Path(__file__).resolve().parents[2]
SOURCES = Path(__file__).with_name("sources.json")
CACHE = ROOT / "data/generated/sogou"
INSTALL = Path(os.environ.get("XDG_DATA_HOME", Path.home() / ".local/share")) / "qingjian/dicts"
PINYIN = re.compile(r"[a-z]+(?:'[a-z]+)*\Z")
MAGIC = bytes.fromhex("40150000")


def fetch(url: str, referer: str | None = None) -> bytes:
    headers = {"User-Agent": "Mozilla/5.0 (compatible; QingjianSogouImport/1.0)"}
    if referer:
        headers["Referer"] = referer
    with urlopen(Request(url, headers=headers), timeout=30) as response:
        if response.status != 200:
            raise RuntimeError(f"下载失败：HTTP {response.status} {url}")
        data = response.read(10_000_001)
    if len(data) > 10_000_000:
        raise RuntimeError(f"文件超过 10 MB：{url}")
    return data


def page_info(source: dict) -> tuple[str, str, int]:
    page = f"https://pinyin.sogou.com/dict/detail/index/{source['id']}"
    body = fetch(page).decode("utf-8")
    title_match = re.search(r'<div class="dict_detail_title">\s*(.*?)\s*</div>', body, re.S)
    link_match = re.search(r'<div id="dict_dl_btn">\s*<a[^>]+href="([^"]+)"', body)
    plain = html.unescape(re.sub(r"<[^>]+>", " ", body)).replace("\xa0", " ")
    count_match = re.search(r"词\s*条\s*[：:]\s*(\d+)个", plain)
    if not title_match or not link_match or not count_match:
        raise RuntimeError(f"无法读取搜狗词库信息：{page}")
    title = html.unescape(title_match.group(1))
    if title != source["title"]:
        raise RuntimeError(f"词库标题变化，需核对来源：{page} ({title})")
    download = quote(urljoin(page, html.unescape(link_match.group(1))), safe=":/?&=%")
    parsed = urlparse(download)
    if parsed.scheme != "https" or parsed.netloc != "pinyin.sogou.com" or parsed.path != "/d/dict/download_cell.php":
        raise RuntimeError(f"下载地址异常：{download}")
    if f"id={source['id']}" not in parsed.query:
        raise RuntimeError(f"下载词库编号异常：{download}")
    return page, download, int(count_match.group(1))


def convert(org: Path, page: str, title: str) -> tuple[bytes, dict]:
    accepted: dict[tuple[str, str], None] = {}
    skipped: dict[str, int] = {}
    raw_entries = 0
    for line in org.read_text(encoding="utf-8").splitlines():
        if line.startswith(("DESC:", "SOURCE:", "LONGDESC:", "EXAMPLE:")):
            continue
        raw_entries += 1
        parts = line.split("\t")
        reason = ""
        if len(parts) != 3:
            reason = "columns"
        else:
            word, code, _ = parts
            syllables = code.split("'")
            if not PINYIN.fullmatch(code):
                reason = "pinyin"
            elif len(syllables) > 8:
                reason = "too_long"
            elif not word or not all("\u3400" <= char <= "\u9fff" for char in word):
                reason = "non_han"
            elif len(word) != len(syllables):
                reason = "syllables"
            else:
                accepted[(word, " ".join(syllables))] = None
        if reason:
            skipped[reason] = skipped.get(reason, 0) + 1
    if not accepted:
        raise RuntimeError(f"词库没有可用词条：{page}")
    lines = [f"# 来源：{page}", f"# 搜狗词库：{title}", "# 词\t拼音\t词频"]
    lines.extend(f"{word}\t{pinyin}\t10" for word, pinyin in sorted(accepted))
    data = ("\n".join(lines) + "\n").encode("utf-8")
    return data, {"raw_entries": raw_entries, "installed_entries": len(accepted), "skipped": skipped}


def write_atomic(path: Path, data: bytes, replace: bool) -> str:
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.exists():
        if path.read_bytes() == data:
            return "already_current"
        if not replace:
            raise RuntimeError(f"目标文件已修改，未覆盖：{path}；确认后可加 --replace")
    fd, temporary = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    try:
        os.fchmod(fd, 0o600)
        with os.fdopen(fd, "wb") as out:
            out.write(data)
            out.flush()
            os.fsync(out.fileno())
        os.replace(temporary, path)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)
    return "installed"


def run() -> None:
    parser = argparse.ArgumentParser(description="下载搜狗 .scel 并安装为青简附加词库")
    parser.add_argument("--replace", action="store_true", help="覆盖之前由此脚本安装、但内容不同的词库")
    parser.add_argument("--cache-dir", type=Path, default=CACHE)
    parser.add_argument("--install-dir", type=Path, default=INSTALL)
    args = parser.parse_args()
    if not shutil.which("scel2org5"):
        raise SystemExit("缺少 scel2org5，请安装 fcitx5-chinese-addons-bin")
    sources = json.loads(SOURCES.read_text(encoding="utf-8"))
    args.cache_dir.mkdir(parents=True, exist_ok=True)
    report = []
    for source in sources:
        page, download, expected = page_info(source)
        name = f"sogou-{source['id']}-{source['slug']}"
        scel = args.cache_dir / f"{name}.scel"
        data = fetch(download, page)
        if not data.startswith(MAGIC):
            raise RuntimeError(f"下载内容不是 .scel：{page}")
        scel.write_bytes(data)
        org = args.cache_dir / f"{name}.org"
        subprocess.run(["scel2org5", "-o", str(org), str(scel)], check=True, capture_output=True)
        converted, counts = convert(org, page, source["title"])
        if abs(counts["raw_entries"] - expected) > max(5, expected // 100):
            raise RuntimeError(f"词条数与搜狗页面差异过大：{page}，页面 {expected}、实际 {counts['raw_entries']}")
        output = args.cache_dir / f"{name}.tsv"
        output.write_bytes(converted)
        target = args.install_dir / output.name
        state = write_atomic(target, converted, args.replace)
        info = {"id": source["id"], "topic": source["topic"], "title": source["title"], "page": page,
                "sha256": hashlib.sha256(data).hexdigest(), "bytes": len(data), **counts, "state": state,
                "installed_path": str(target)}
        report.append(info)
        print(f"{source['topic']} {source['title']}：{counts['installed_entries']}/{expected} 条，{state}")
    (args.cache_dir / "manifest.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(f"共安装 {sum(item['installed_entries'] for item in report)} 条；清单：{args.cache_dir / 'manifest.json'}")


if __name__ == "__main__":
    run()
