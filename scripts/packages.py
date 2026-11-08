#!/usr/bin/env python3
"""Write the Homebrew formula and cask and the Scoop manifest for one release.

Usage: packages.py <version> <dist folder>

The dist folder holds the release files, so the sha256 sums match what was
uploaded. Files that are missing (a Mac job that failed, say) leave that
package untouched, so it keeps pointing at the last good release.
"""

import hashlib
import json
import pathlib
import sys

REPO = "kajisho5/hushtv"
DESC = "Switch off ads, trackers and promo apps on Android TV and Google TV - undoably"
HOME = f"https://github.com/{REPO}"


def sha(path: pathlib.Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def url(version: str, name: str) -> str:
    return f"{HOME}/releases/download/v{version}/{name}"


def formula(version: str, dist: pathlib.Path) -> str | None:
    files = {
        "mac": "hushtv-macos-universal.zip",
        "linux_intel": "hushtv-linux-x86_64.tar.gz",
        "linux_arm": "hushtv-linux-arm64.tar.gz",
    }
    if not all((dist / f).is_file() for f in files.values()):
        return None
    s = {k: sha(dist / f) for k, f in files.items()}
    u = {k: url(version, f) for k, f in files.items()}
    return f'''# Written by scripts/packages.py on every release. Don't edit by hand.
class Hushtv < Formula
  desc "{DESC}"
  homepage "{HOME}"
  version "{version}"
  license "MIT"

  on_macos do
    url "{u['mac']}"
    sha256 "{s['mac']}"
  end

  on_linux do
    on_intel do
      url "{u['linux_intel']}"
      sha256 "{s['linux_intel']}"
    end
    on_arm do
      url "{u['linux_arm']}"
      sha256 "{s['linux_arm']}"
    end
  end

  def install
    bin.install "hushtv"
  end

  test do
    assert_match version.to_s, shell_output("#{{bin}}/hushtv --version")
  end
end
'''


def cask(version: str, dist: pathlib.Path) -> str | None:
    name = f"hushtv_{version}_macos-universal.dmg"
    if not (dist / name).is_file():
        return None
    return f'''# Written by scripts/packages.py on every release. Don't edit by hand.
cask "hushtv-app" do
  version "{version}"
  sha256 "{sha(dist / name)}"

  url "{HOME}/releases/download/v#{{version}}/hushtv_#{{version}}_macos-universal.dmg"
  name "hushtv"
  desc "{DESC}"
  homepage "{HOME}"

  depends_on macos: ">= :high_sierra"

  app "hushtv.app"

  zap trash: [
    "~/Library/Application Support/io.github.kajisho5.hushtv",
    "~/Library/Caches/io.github.kajisho5.hushtv",
    "~/Library/WebKit/io.github.kajisho5.hushtv",
  ]
end
'''


def scoop(version: str, dist: pathlib.Path) -> str | None:
    name = "hushtv-windows-x86_64.zip"
    if not (dist / name).is_file():
        return None
    manifest = {
        "version": version,
        "description": DESC,
        "homepage": HOME,
        "license": "MIT",
        "architecture": {
            "64bit": {
                "url": url(version, name),
                "hash": sha(dist / name),
                "extract_dir": "hushtv-windows-x86_64",
            }
        },
        "bin": "hushtv.exe",
        "checkver": {"github": HOME},
        "autoupdate": {
            "architecture": {
                "64bit": {"url": f"{HOME}/releases/download/v$version/{name}"}
            }
        },
    }
    return json.dumps(manifest, indent=4, ensure_ascii=False) + "\n"


def updater(version: str, dist: pathlib.Path) -> str | None:
    """Tauri の自動アップデートが読む latest.json。署名（.sig）がそろっているものだけ載せる。"""
    import datetime

    files = {
        "darwin-aarch64": f"hushtv_{version}_macos-universal.app.tar.gz",
        "darwin-x86_64": f"hushtv_{version}_macos-universal.app.tar.gz",
        "linux-x86_64": f"hushtv_{version}_amd64.AppImage",
        "windows-x86_64": f"hushtv_{version}_x64-setup.exe",
    }
    platforms = {}
    for key, name in files.items():
        sig = dist / f"{name}.sig"
        if (dist / name).is_file() and sig.is_file():
            platforms[key] = {"signature": sig.read_text().strip(), "url": url(version, name)}
    if not platforms:
        return None
    notes_file = pathlib.Path(__file__).resolve().parent.parent / "docs" / "release-notes.md"
    notes = ""
    if notes_file.is_file():
        # 英語の段落と日本語の段落を分けて入れる（アプリが言語に合わせて選ぶ）
        en, ja, in_ja = [], [], False
        for line in notes_file.read_text().splitlines():
            if not in_ja and any(ord(c) > 0x2E7F for c in line) and not line.startswith(("-", "*", "New")):
                in_ja = True
            (ja if in_ja else en).append(line)
        notes = "\n".join(en).strip() + "\n---ja---\n" + "\n".join(ja).strip()
    manifest = {
        "version": version,
        "notes": notes,
        "pub_date": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "platforms": platforms,
    }
    return json.dumps(manifest, indent=2, ensure_ascii=False) + "\n"


def main() -> int:
    version, dist = sys.argv[1], pathlib.Path(sys.argv[2])
    root = pathlib.Path(__file__).resolve().parent.parent
    out = {
        root / "Formula" / "hushtv.rb": formula(version, dist),
        root / "Casks" / "hushtv-app.rb": cask(version, dist),
        root / "bucket" / "hushtv.json": scoop(version, dist),
        root / "updater" / "latest.json": updater(version, dist),
    }
    for path, text in out.items():
        if text is None:
            print(f"skip {path.relative_to(root)} (files missing)")
            continue
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)
        print(f"wrote {path.relative_to(root)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
