#!/usr/bin/env python3
"""cleanyourtv の機種別リストを data/lists/ に取り込む。

    python3 -I scripts/sync_upstream.py <cleanyourtv をクローンしたフォルダ>

examples/<機種>/ の debloat*.txt と tv.json だけを写し、取り込んだコミットを data/UPSTREAM.json に書く。
"""
import json
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DEST = ROOT / "data" / "lists"


def main():
    if len(sys.argv) != 2:
        raise SystemExit(__doc__)
    src = Path(sys.argv[1]).resolve()
    examples = src / "examples"
    if not examples.is_dir():
        raise SystemExit(f"examples/ がありません: {src}")
    commit = subprocess.run(["git", "-C", str(src), "rev-parse", "HEAD"], capture_output=True,
                            text=True, check=True).stdout.strip()
    if DEST.exists():
        shutil.rmtree(DEST)
    count = 0
    for folder in sorted(p for p in examples.iterdir() if p.is_dir()):
        files = sorted(folder.glob("debloat*.txt"))
        if not files:
            continue
        out = DEST / folder.name
        out.mkdir(parents=True)
        for f in files + [folder / "tv.json"]:
            if f.is_file():
                shutil.copyfile(f, out / f.name)
        count += 1
    shutil.copyfile(src / "LICENSE", ROOT / "data" / "LICENSE-cleanyourtv")
    (ROOT / "data" / "UPSTREAM.json").write_text(json.dumps(
        {"repo": "Gathin23/cleanyourtv", "commit": commit, "lists": count}, indent=2) + "\n")
    print(f"{count} 機種のリストを取り込みました（{commit[:12]}）")


if __name__ == "__main__":
    main()
