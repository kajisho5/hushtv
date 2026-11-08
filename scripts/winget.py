#!/usr/bin/env python3
"""winget（Windows の公式のアプリ一覧）に出す定義を書く。

Usage: winget.py <version> <setup.exe> <out folder>

できた 3 つのファイルを wingetcreate submit で microsoft/winget-pkgs に送る（release.yml）。
"""

import hashlib
import pathlib
import sys

ID = "kajisho5.hushtv"
SCHEMA = "1.6.0"
HOME = "https://github.com/kajisho5/hushtv"


def main() -> int:
    version, exe, out = sys.argv[1], pathlib.Path(sys.argv[2]), pathlib.Path(sys.argv[3])
    sha = hashlib.sha256(exe.read_bytes()).hexdigest().upper()
    url = f"{HOME}/releases/download/v{version}/{exe.name}"
    head = f"# yaml-language-server: $schema=https://aka.ms/winget-manifest.{{kind}}.{SCHEMA}.schema.json\n"
    files = {
        f"{ID}.yaml": head.format(kind="version") + f"""PackageIdentifier: {ID}
PackageVersion: {version}
DefaultLocale: en-US
ManifestType: version
ManifestVersion: {SCHEMA}
""",
        f"{ID}.installer.yaml": head.format(kind="installer") + f"""PackageIdentifier: {ID}
PackageVersion: {version}
InstallerType: nullsoft
Scope: user
UpgradeBehavior: install
Installers:
  - Architecture: x64
    InstallerUrl: {url}
    InstallerSha256: {sha}
ManifestType: installer
ManifestVersion: {SCHEMA}
""",
        f"{ID}.locale.en-US.yaml": head.format(kind="defaultLocale") + f"""PackageIdentifier: {ID}
PackageVersion: {version}
PackageLocale: en-US
Publisher: Shogo Kajiyama
PublisherUrl: https://github.com/kajisho5
PackageName: hushtv
PackageUrl: {HOME}
License: MIT
LicenseUrl: {HOME}/blob/main/LICENSE
ShortDescription: Switch off ads, trackers and promo apps on Android TV and Google TV, undoably.
Description: hushtv switches off preinstalled ads, trackers and promo apps on Android TV and Google TV over Wi-Fi. Nothing is uninstalled, every change can be undone, and safety checks keep the TV bootable. Lists for 456 TV models from cleanyourtv are built in.
Tags:
  - android-tv
  - google-tv
  - adb
  - debloat
  - privacy
ReleaseNotesUrl: {HOME}/releases/tag/v{version}
ManifestType: defaultLocale
ManifestVersion: {SCHEMA}
""",
    }
    out.mkdir(parents=True, exist_ok=True)
    for name, text in files.items():
        (out / name).write_text(text, encoding="utf-8")
        print("wrote", out / name)
    return 0


if __name__ == "__main__":
    sys.exit(main())
