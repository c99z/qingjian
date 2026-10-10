#!/usr/bin/env python3
# //! 从应用图标母版生成 Windows ICO 与 Linux 各尺寸 PNG；需要 Pillow。
from pathlib import Path

from PIL import Image


root = Path(__file__).resolve().parents[2]
icon_dir = root / 'assets/icon'
with Image.open(icon_dir / 'logo.png') as source:
    image = source.convert('RGBA')
    if image.width != image.height:
        raise SystemExit('应用图标母版必须为正方形')
    for size in (16, 24, 32, 48, 64, 128, 256, 512, 1024):
        destination = icon_dir / 'linux' / f'{size}x{size}' / 'apps/qingjian.png'
        destination.parent.mkdir(parents=True, exist_ok=True)
        image.resize((size, size), Image.Resampling.LANCZOS).save(destination, optimize=True)
    image.save(
        root / 'apps/windows/tsf/resources/qingjian.ico',
        sizes=[(size, size) for size in (16, 20, 24, 32, 40, 48, 64, 128, 256)],
    )
print('已生成 Linux 多尺寸 PNG 与 Windows ICO')
