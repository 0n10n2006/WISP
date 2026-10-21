"""Packages rendered brand PNGs into the files Tauri and the installers use.

Usage: python3 scripts/brand/package.py <renderDir>
"""
import shutil
import sys
from pathlib import Path

from PIL import Image

src = Path(sys.argv[1])
icons = Path("src-tauri/icons")
installer = Path("src-tauri/installer")
installer.mkdir(parents=True, exist_ok=True)

# Windows .ico: hand-tuned small sizes, full mark for larger ones.
sizes = [16, 24, 32, 48, 64, 128, 256]
images = [Image.open(src / f"icon-{s}.png").convert("RGBA") for s in sizes]
images[-1].save(icons / "icon.ico", format="ICO", sizes=[(s, s) for s in sizes], append_images=images[:-1])

# PNG set referenced by tauri.conf.json.
Image.open(src / "icon-32.png").save(icons / "32x32.png")
Image.open(src / "icon-128.png").save(icons / "128x128.png")
Image.open(src / "icon-256.png").save(icons / "128x128@2x.png")
Image.open(src / "icon-1024.png").save(icons / "icon.png")
for name in ["tray-light-16", "tray-light-32", "tray-dark-16", "tray-dark-32"]:
    shutil.copy(src / f"{name}.png", icons / f"{name}.png")

# Installer art must be 24-bit BMP.
for name in ["nsis-sidebar", "nsis-header", "wix-banner", "wix-dialog"]:
    Image.open(src / f"{name}.png").convert("RGB").save(installer / f"{name}.bmp")
print("packaged icons and installer art")
