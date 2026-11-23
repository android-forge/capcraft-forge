#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 CapCraft contributors
"""Packs the graphics library the app searches: graphics/build.py [cache folder]

Reads pinned releases of open icon sets (as Iconify packages them) from the npm registry, our own
shapes from own.py beside this, and the words to find them by, and writes

  src/crates/capcraft-android/android/app/src/main/assets/graphics/index.bin   names, words, where each is
  src/crates/capcraft-android/android/app/src/main/assets/graphics/shapes.bin  their SVG bodies, in deflated blocks
  graphics/SOURCES.md                                                          what is in it, and under what licence

Both files are deflated (raw, level 9). The index is lines: "#" id, title, licence, author, link, whether a credit
is asked for, whether it takes one colour; "@" offset and length of each block; then a line for each
graphic: name, words, width, height, block, offset in it, length.
"""
import io, json, os, re, sys, tarfile, urllib.request, zlib

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "../src/crates/capcraft-android/android/app/src/main/assets/graphics")
CACHE = sys.argv[1] if len(sys.argv) > 1 else os.path.join(HERE, "cache")
BLOCK = 192 * 1024

# id, version, title, one colour, which of it
SETS = [
    ("fluent-emoji-flat", "1.2.6", "Emoji", False, None),
    ("streamline-stickies-color", "1.2.2", "Stickies", False, None),
    ("streamline-kameleon-color", "1.2.3", "Circles", False, None),
    ("streamline-ultimate-color", "1.2.2", "Colour", False, lambda n: not n.endswith("-logo") and "-logo-" not in n),
    ("streamline-plump-color", "1.2.2", "Plump", False, lambda n: "logo" not in n and not n.endswith("-flat")),
    ("streamline-freehand-color", "1.2.2", "Freehand", False, lambda n: "logo" not in n),
    ("streamline-cyber-color", "1.2.2", "Cyber", False, None),
    ("fluent-color", "1.2.21", "Gradient", False, lambda n: re.search(r"-(48|32|28|24)$", n)),
    ("flat-color-icons", "1.2.3", "Flat", False, None),
    ("marketeq", "1.2.2", "Two tone", False, None),
    ("circle-flags", "1.2.14", "Flags", False, lambda n: not n.startswith("lang-")),
    ("logos", "1.2.20", "Brands", False, "brands"),
    ("ph", "1.2.2", "Symbols", True, lambda n: not re.search(r"-(thin|light|bold)$", n)),
    ("tabler", "1.2.46", "Outline", True, lambda n: not n.startswith("brand-")),
    ("healthicons", "1.2.18", "Health", True, lambda n: not n.endswith("-outline") and not n.endswith("-24px") and "negative" not in n),
]
CREDIT = {"CC-BY-4.0", "CC-BY-3.0"}
BRANDS = """youtube youtube-icon instagram instagram-icon facebook whatsapp whatsapp-icon tiktok tiktok-icon x telegram
snapchat snapchat-icon twitch linkedin linkedin-icon pinterest reddit reddit-icon discord discord-icon spotify spotify-icon
google google-icon google-play google-play-icon google-play-store google-drive google-gmail google-maps google-meet
google-photos google-pay apple apple-app-store apple-pay android android-icon netflix netflix-icon
amazon amazon-icon microsoft microsoft-icon microsoft-windows-icon microsoft-edge chrome firefox safari opera messenger signal
threads bluesky mastodon mastodon-icon medium medium-icon tumblr tumblr-icon quora vimeo vimeo-icon soundcloud
patreon paypal visa mastercard stripe shopify airbnb airbnb-icon uber slack slack-icon zoom zoom-icon skype notion notion-icon
figma dribbble dribbble-icon behance github github-icon wordpress wordpress-icon dropbox adobe adobe-photoshop adobe-premiere
adobe-after-effects adobe-illustrator adobe-lightroom openai openai-icon claude claude-icon bitcoin ethereum-color
steam playstation xbox nintendo unity epic-games wikipedia yahoo bing duckduckgo flickr flickr-icon
kickstarter kickstarter-icon product-hunt buffer hootsuite mailchimp trello line wechat weibo vk""".split()


# what people look for, beside what a graphic is called
SYNONYMS = {
    "firework": ["diwali", "newyear"], "fireworks": ["diwali", "newyear"], "firecracker": ["diwali"], "diya": ["diwali", "deepavali"], "sparkler": ["diwali"],
    "bell": ["subscribe", "notification", "alert"], "heart": ["love", "like"], "thumbs": ["like"], "automobile": ["car"],
    "television": ["tv"], "tv": ["television"], "motorcycle": ["bike"], "bicycle": ["bike"], "cellphone": ["mobile"], "smartphone": ["mobile", "phone"],
    "airplane": ["plane", "flight", "aeroplane"], "film": ["movie", "cinema"], "movie": ["film", "cinema"], "rupee": ["inr", "india"],
    "ballot": ["vote", "election"], "gift": ["present"], "present": ["gift"], "dollar": ["money", "usd"], "bulb": ["idea"], "trophy": ["winner", "win"],
    "mosque": ["muslim", "islam", "eid"], "church": ["christian"], "temple": ["hindu"], "turban": ["sikh"], "cake": ["birthday"], "balloon": ["birthday", "party"],
    "soccer": ["football"], "couple": ["wedding", "marriage"], "ring": ["wedding", "engagement"], "sun": ["summer", "weather"], "snowflake": ["winter", "cold"],
    "umbrella": ["rain", "monsoon"], "curry": ["indian", "food"], "sari": ["saree", "indian"], "mask": ["theatre"], "microphone": ["podcast", "sing", "mic"],
}


def package(name, version):
    """The files of an npm package, as {path: bytes}."""
    os.makedirs(CACHE, exist_ok=True)
    kept = os.path.join(CACHE, name.replace("/", "_") + "-" + version + ".tgz")
    if not os.path.exists(kept):
        url = f"https://registry.npmjs.org/{name}/-/{name.split('/')[-1]}-{version}.tgz"
        with urllib.request.urlopen(url) as r, open(kept + ".part", "wb") as f:
            f.write(r.read())
        os.rename(kept + ".part", kept)
    files = {}
    with tarfile.open(kept) as t:
        for m in t:
            if m.isfile() and m.size < 60_000_000 and (m.name.endswith((".json", ".mjs")) or "LICENSE" in m.name):
                files[m.name.split("/", 1)[1]] = t.extractfile(m).read()
    return files


def words_of(name):
    return [w for w in re.split(r"[-_ ]+", name) if w and not w.isdigit()]


def extra_words():
    """What else things are called: from Tabler's and Phosphor's own tags, and Unicode's for emoji."""
    words = {}

    def add(key, more):
        have = words.setdefault(key, set())
        for w in more:
            w = str(w).lower().strip().replace("_", "-")
            if re.fullmatch(r"[a-z0-9' -]{2,30}", w):
                have.update(words_of(w))

    tabler = json.loads(package("@tabler/icons", "3.52.0")["icons.json"])
    for name, icon in tabler.items():
        add("tabler:" + name, icon.get("tags", []) + [icon.get("category", "")])
    for name, tags in json.loads(package("lucide-static", "1.77.0")["tags.json"]).items():
        add("plain:" + name, tags)
    core = package("@phosphor-icons/core", "2.1.1")
    text = next((v.decode() for k, v in core.items() if k.endswith(".mjs")), "")
    for m in re.finditer(r'name:\s*"([a-z0-9-]+)".*?tags:\s*\[(.*?)\]', text, re.S):
        add("ph:" + m.group(1), re.findall(r'"([^"]+)"', m.group(2)))
    cldr = json.loads(package("cldr-annotations-full", "49.0.0")["annotations/en/annotations.json"])
    for char, a in cldr["annotations"]["annotations"].items():
        code = "-".join(f"{ord(c):x}" for c in char if c != "️")
        add("emoji:" + code, a.get("default", []) + a.get("tts", []))
    return words


def tidy(body):
    """An SVG body without what a still picture has no use for, its figures no finer than shows."""
    body = re.sub(r"<(animate|animateTransform|animateMotion|set)\b[^>]*?(/>|>.*?</\1>)", "", body, flags=re.S)
    return body.replace("\n", "").replace("\t", "")


def main():
    extra = extra_words()
    names = json.loads(package("cldr-localenames-full", "49.0.0")["main/en/territories.json"])
    countries = names["main"]["en"]["localeDisplayNames"]["territories"]
    blocks, where = [], []
    current, lines, totals = bytearray(), [], []

    def put(body):
        nonlocal current
        data = body.encode()
        if len(current) + len(data) > BLOCK and current:
            blocks.append(bytes(current))
            current = bytearray()
        at = len(current)
        current += data
        return len(blocks), at, len(data)

    def close():
        nonlocal current
        if current:
            blocks.append(bytes(current))
            current = bytearray()

    sys.path.insert(0, HERE)
    import own
    lines.append("\t".join(["#", "own", "Video", "AGPL-3.0-or-later", "CapCraft contributors", "", "0", "0"]))
    n = 0
    for name, words, w, h, body in own.shapes():
        b, at, size = put(body)
        lines.append("\t".join([name, words, f"{w:g}", f"{h:g}", str(b), str(at), str(size)]))
        n += 1
    totals.append(("own", "Video", n, "AGPL-3.0-or-later", "CapCraft contributors", ""))
    close()
    for set_id, version, title, mono, keep in SETS:
        files = package("@iconify-json/" + set_id, version)
        icons = json.loads(files["icons.json"])
        info = json.loads(files["info.json"])
        meta = json.loads(files.get("metadata.json", b"{}"))
        kinds = {}
        for kind, names in (meta.get("categories") or {}).items():
            for name in names:
                kinds.setdefault(name, []).extend(words_of(kind.lower().replace("&", "").replace(",", "")))
        chars = {}
        for code, name in json.loads(files.get("chars.json", b"{}")).items():
            chars.setdefault(name, code)
        licence = info["license"]["spdx"]
        # nothing is taken whose licence this repository could not carry
        assert licence in {"MIT", "ISC", "Apache-2.0", "CC0-1.0", "CC-BY-4.0"}, (set_id, licence)
        lines.append("\t".join(["#", set_id, title, licence, info["author"]["name"], info["author"].get("url", ""), "1" if licence in CREDIT else "0", "1" if mono else "0"]))
        n = 0
        seen = set()
        # a shelf opens on the plain ones: the skin tones follow
        tone = re.compile(r"-(light|medium-light|medium|medium-dark|dark)$")
        for name, icon in sorted(icons["icons"].items(), key=lambda i: bool(chars and tone.search(i[0]))):
            if icon.get("hidden"):
                continue
            if keep == "brands":
                if name not in BRANDS:
                    continue
            elif keep is not None and not keep(name):
                continue
            plain = name
            if set_id == "fluent-color":
                # the largest of each only
                plain = re.sub(r"-(16|20|24|28|32|48)$", "", name)
                best = max((int(m.group(1)) for k in icons["icons"] for m in [re.fullmatch(re.escape(plain) + r"-(\d+)", k)] if m), default=0)
                if not name.endswith(f"-{best}"):
                    continue
            if plain in seen:
                continue
            seen.add(plain)
            words = list(words_of(plain)) + kinds.get(name, [])
            base = re.sub(r"-(fill|duotone|filled|off)$", "", plain)
            words += sorted(extra.get("plain:" + base, ()))
            if set_id == "tabler":
                words += sorted(extra.get("tabler:" + base, ()))
            if set_id == "ph":
                words += sorted(extra.get("ph:" + base, ()))
            if name in chars:
                words += sorted(extra.get("emoji:" + chars[name], ()))
            if set_id == "circle-flags":
                words += ["flag", "country"] + words_of(countries.get(plain.upper(), "").lower().replace("&", "").replace("\u2019", ""))
            for have, more in SYNONYMS.items():
                if have in words:
                    words += more
            if set_id == "logos":
                words += ["logo", "brand", "social"]
            # each word once, the name's own first
            words = list(dict.fromkeys(w.lower() for w in words))
            w = icon.get("width", icons.get("width", 16))
            h = icon.get("height", icons.get("height", 16))
            left, top = icon.get("left", icons.get("left", 0)), icon.get("top", icons.get("top", 0))
            body = tidy(icon["body"])
            if left or top:
                body = f'<g transform="translate({-left:g} {-top:g})">{body}</g>'
            b, at, size = put(body)
            lines.append("\t".join([plain, " ".join(words[len(words_of(plain)):]), f"{w:g}", f"{h:g}", str(b), str(at), str(size)]))
            n += 1
        close()
        totals.append((set_id, title, n, licence, info["author"]["name"], info["author"].get("url", "")))

    os.makedirs(OUT, exist_ok=True)
    out, offset = io.BytesIO(), 0
    for raw in blocks:
        z = zlib.compressobj(9, zlib.DEFLATED, -15, 9)
        data = z.compress(raw) + z.flush()
        where.append(f"@\t{offset}\t{len(data)}\t{len(raw)}")
        out.write(data)
        offset += len(data)
    with open(os.path.join(OUT, "shapes.bin"), "wb") as f:
        f.write(out.getvalue())
    z = zlib.compressobj(9, zlib.DEFLATED, -15, 9)
    index = z.compress(("\n".join(where + lines) + "\n").encode()) + z.flush()
    with open(os.path.join(OUT, "index.bin"), "wb") as f:
        f.write(index)
    with open(os.path.join(HERE, "SOURCES.md"), "w") as f:
        f.write("# The graphics library\n\nWhat `build.py` packs into the app, and whose it is. Brand logos are their owners' trademarks and stay so.\n\n")
        f.write("| Shelf | Graphics | Set | By | Licence |\n|---|---|---|---|---|\n")
        for set_id, title, n, licence, author, url in totals:
            version = next((v for i, v, *_ in SETS if i == set_id), "")
            name = f"[{set_id}](https://www.npmjs.com/package/@iconify-json/{set_id}) {version}" if version else "own.py"
            f.write(f"| {title} | {n} | {name} | {'[' + author + '](' + url + ')' if url else author} | {licence} |\n")
        f.write("\nThe words they are found by come from the sets themselves, [Tabler Icons](https://github.com/tabler/tabler-icons) (MIT), [Lucide](https://github.com/lucide-icons/lucide) (ISC), [Phosphor](https://github.com/phosphor-icons/core) (MIT) and [Unicode CLDR](https://cldr.unicode.org/) (Unicode-3.0).\n")
        f.write("\nGraphics under CC BY 4.0 ask for a credit where they are shown; the app says which do and copies the credit.\n")
    mit = "Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the \"Software\"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:\n\nThe above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.\n\nTHE SOFTWARE IS PROVIDED \"AS IS\", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE."
    isc = "Permission to use, copy, modify, and/or distribute this software for any purpose with or without fee is hereby granted, provided that the above copyright notice and this permission notice appear in all copies.\n\nTHE SOFTWARE IS PROVIDED \"AS IS\" AND THE AUTHOR DISCLAIMS ALL WARRANTIES WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE."
    with open(os.path.join(HERE, "../licences/GRAPHICS.txt"), "w") as f:
        f.write("The graphics library. Brand logos are their owners' trademarks and stay so.\n")
        for set_id, title, n, licence, author, url in totals:
            f.write(f"\n{title} ({set_id}): {n} graphics, {licence}\nCopyright (c) {author}{' - ' + url if url else ''}\n")
        f.write("\nCC-BY-4.0: https://creativecommons.org/licenses/by/4.0/legalcode - the graphics are as their authors made them, packed together. Credit them where they are shown.\n")
        f.write("CC0-1.0: https://creativecommons.org/publicdomain/zero/1.0/legalcode\n")
        f.write("\nThe words graphics are found by:\nTabler Icons, MIT, Copyright (c) 2020-2026 Pawel Kuna\nPhosphor Icons, MIT, Copyright (c) 2023 Phosphor Icons\nLucide, ISC, Copyright (c) Lucide Icons and Contributors\nUnicode CLDR, Unicode-3.0, Copyright (c) 1991-2026 Unicode, Inc. - https://www.unicode.org/license.txt\n")
        f.write("\n\nMIT\n\n" + mit + "\n\n\nISC\n\n" + isc + "\n")
    print(sum(t[2] for t in totals), "graphics", len(index), "+", offset, "bytes")


main()
