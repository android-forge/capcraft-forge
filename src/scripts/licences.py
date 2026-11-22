#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Writes licences/: the licence texts a release carries beside its own.

    src/scripts/licences.py [ffmpeg-source-folder]

RUST_CRATES.txt is every crate the phone's library links, with the licence
each names and the texts its package holds; FONTS.txt is every font's. The
GPL and LGPL texts are FFmpeg's copies, when its source folder is given.
Run it when Cargo.lock or the fonts change, and before every release.
"""
import json, pathlib, shutil, subprocess, sys

src = pathlib.Path(__file__).resolve().parent.parent
out = src.parent / "licences"
out.mkdir(exist_ok=True)

meta = json.loads(subprocess.run(
    ["cargo", "metadata", "--format-version", "1", "--locked", "--filter-platform", "aarch64-linux-android"],
    cwd=src, check=True, capture_output=True, text=True).stdout)
packages = {p["id"]: p for p in meta["packages"]}
nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}
root = next(p["id"] for p in meta["packages"] if p["name"] == "capcraft-android")
linked, todo = set(), [root]
while todo:
    here = todo.pop()
    if here in linked:
        continue
    linked.add(here)
    for dep in nodes[here]["deps"]:
        if any(k["kind"] is None for k in dep["dep_kinds"]):
            todo.append(dep["pkg"])
ours = {p["id"] for p in meta["packages"] if p["source"] is None}

texts, order, bare = {}, [], []
for id in sorted(linked - ours, key=lambda i: (packages[i]["name"], packages[i]["version"])):
    p = packages[id]
    folder = pathlib.Path(p["manifest_path"]).parent
    found = sorted(f for f in folder.iterdir() if f.is_file() and f.name.upper().split(".")[0].split("-")[0] in
                   ("LICENSE", "LICENCE", "COPYING", "NOTICE", "COPYRIGHT", "UNLICENSE"))
    order.append(p)
    if not found:
        bare.append(p)
    for f in found:
        texts.setdefault(f.read_text(errors="replace").strip(), []).append(f"{p['name']} {p['version']}")
with open(out / "RUST_CRATES.txt", "w") as f:
    f.write("The Rust crates linked into the app, and the licence each names.\n\n")
    for p in order:
        f.write(f"{p['name']} {p['version']} - {p['license'] or p['license_file'] or 'see its repository'} - {p['repository'] or ''}\n")
    f.write("\n\nTheir licence texts.\n")
    for text, names in texts.items():
        f.write("\n" + "=" * 72 + "\n" + ", ".join(names) + "\n" + "-" * 72 + "\n" + text + "\n")

fonts = src / "crates/concat-text/fonts"
with open(out / "FONTS.txt", "w") as f:
    f.write("The fonts the app carries, and the licence of each.\n")
    for licence in sorted(fonts.glob("LICENSE-*.txt")):
        f.write("\n" + "=" * 72 + f"\n{licence.stem.removeprefix('LICENSE-')}\n" + "-" * 72 + "\n" + licence.read_text(errors="replace").strip() + "\n")

if len(sys.argv) > 1:
    ffmpeg = pathlib.Path(sys.argv[1])
    shutil.copy(ffmpeg / "COPYING.LGPLv2.1", out / "LGPL-2.1.txt")
    shutil.copy(ffmpeg / "COPYING.GPLv3", out / "GPL-3.0.txt")

licences = {}
for p in order:
    licences[p["license"] or "(none named)"] = licences.get(p["license"] or "(none named)", 0) + 1
print(f"{len(order)} crates, {len(texts)} texts, {len(bare)} without a text of their own")
for name, n in sorted(licences.items(), key=lambda x: -x[1]):
    print(f"  {n:4} {name}")
print("without:", ", ".join(f"{p['name']} ({p['license']})" for p in bare))
