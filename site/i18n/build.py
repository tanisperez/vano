#!/usr/bin/env python3
"""Builds the translated home and download pages: site/<lang>/index.html and install.html, from
the English pages and i18n/<lang>.json. Standard library only.

    python3 site/i18n/build.py          # writes site/es, site/gl and site/it
    python3 site/i18n/build.py --keys   # prints every text that needs a translation

The English pages are the source. Every text between tags and every alt, aria-label, title and
meta content is looked up in the language's JSON (English text to translation). The build fails,
listing them all, when a text has no translation (the English page changed) or a translation is
no longer used, so the languages never fall behind silently. Texts in KEEP stay as they are.

Inside a translated page, relative links to shared files (css/, img/, docs/...) get a `../`,
links between the two pages stay in the language, and lang, canonical, og:url, og:locale and the
current language link are set.
"""
import json
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
SITE = HERE.parent
BASE = "https://vano.tanis.codes/"
PAGES = ["index.html", "install.html"]
LANGS = {"es": "es_ES", "gl": "gl_ES", "it": "it_IT"}
LOCAL = {"index.html", "install.html", "./"}

# Names, code and file names: the same in every language.
KEEP = {
    "Vano", "GitHub", "tanis.codes", "Estanislao Pérez Nartallo", "English", "Español", "Galego",
    "Italiano", "Windows", "macOS", "Linux", "du", "s", "MB", "Backspace", "vano scan",
    "vano clean", "~/.npm ~/.cargo ~/.gradle", "target/ .next/ build/", "node_modules .venv",
    "node_modules", "Cache Code Cache", "Android/Sdk .ollama", "*.iso *.dmg *.deb",
    "Vano-…-windows-x86_64-setup.exe", "Vano-…-windows-x86_64-portable.zip",
    "Vano-…-macos-arm64.dmg", ".deb", ".rpm", ".tar.gz", "~/.local", "/usr", "bin/", "share/",
    "apt", "dnf, zypper", "tar.gz", "cargo run --release", "README",
}
SKIP_BLOCKS = re.compile(r"<(script|style|pre|svg)\b.*?</\1>", re.S)
TEXT = re.compile(r">([^<>]*[A-Za-zÀ-ÿ][^<>]*)<")
ATTR = re.compile(r'\b(alt|aria-label|title|placeholder)="([^"]*)"')
META = re.compile(r'(<meta (?:name|property)="(?:description|og:title|og:description|og:image:alt|twitter:title|twitter:description|twitter:image:alt)" content=")([^"]*)(")')


def texts(page):
    """Every translatable text of a page, in order."""
    found = []
    body = SKIP_BLOCKS.sub(lambda m: m.group(0) if m.group(1) == "title" else "<x>", page)
    for match in TEXT.finditer(body):
        text = match.group(1).strip()
        if text:
            found.append(text)
    for match in ATTR.finditer(body):
        found.append(match.group(2))
    for match in META.finditer(page):
        found.append(match.group(2))
    return [text for text in found if text not in KEEP and re.search(r"[A-Za-zÀ-ÿ]", text)]


def translate(page, table, used, missing):
    def look(text):
        if text in KEEP or not re.search(r"[A-Za-zÀ-ÿ]", text):
            return text
        if text in table:
            used.add(text)
            return table[text]
        missing.add(text)
        return text

    def text_node(match):
        raw = match.group(1)
        stripped = raw.strip()
        if not stripped:
            return match.group(0)
        lead = raw[: len(raw) - len(raw.lstrip())]
        trail = raw[len(raw.rstrip()):]
        return f">{lead}{look(stripped)}{trail}<"

    # Blocks that are not prose stay untouched: cut them out, translate, put them back.
    blocks = []

    def stash(match):
        blocks.append(match.group(0))
        return f"<\x00{len(blocks) - 1}\x00>"

    page = SKIP_BLOCKS.sub(stash, page)
    page = META.sub(lambda m: m.group(1) + look(m.group(2)) + m.group(3), page)
    page = ATTR.sub(lambda m: f'{m.group(1)}="{look(m.group(2))}"', page)
    page = TEXT.sub(text_node, page)
    return re.sub("<\x00(\\d+)\x00>", lambda m: blocks[int(m.group(1))], page)


def relocate(page, lang, name):
    """Links and metadata of the page as served from /<lang>/."""
    def fix(match):
        attr, url = match.group(1), match.group(2)
        if url in LOCAL or url.startswith(("#", "/", "http:", "https:", "mailto:", "data:")):
            return match.group(0)
        return f'{attr}="../{url}"'

    page = re.sub(r'\b(href|src)="([^"]*)"', fix, page)
    page = page.replace('<html lang="en">', f'<html lang="{lang}">', 1)
    path = "" if name == "index.html" else name
    page = page.replace(
        f'<link rel="canonical" href="{BASE}{path}">',
        f'<link rel="canonical" href="{BASE}{lang}/{path}">',
        1,
    )
    page = page.replace(
        f'<meta property="og:url" content="{BASE}{path}">',
        f'<meta property="og:url" content="{BASE}{lang}/{path}">',
        1,
    )
    page = page.replace(
        '<meta property="og:locale" content="en_US">',
        f'<meta property="og:locale" content="{LANGS[lang]}">',
        1,
    )
    page = page.replace(' data-lang="en" aria-current="true">', ' data-lang="en">', 1)
    page = page.replace(f' data-lang="{lang}">', f' data-lang="{lang}" aria-current="true">', 1)
    return page


def main():
    sources = {name: (SITE / name).read_text() for name in PAGES}
    if "--keys" in sys.argv:
        seen = {}
        for name in PAGES:
            for text in texts(sources[name]):
                seen.setdefault(text, None)
        print(json.dumps({text: "" for text in seen}, ensure_ascii=False, indent=2))
        return

    errors = []
    for lang in LANGS:
        table = json.loads((HERE / f"{lang}.json").read_text())
        used, missing = set(), set()
        out = SITE / lang
        out.mkdir(exist_ok=True)
        for name in PAGES:
            page = translate(sources[name], table, used, missing)
            page = relocate(page, lang, name)
            if "—" in page:
                errors.append(f"{lang}/{name}: contains an em dash")
            (out / name).write_text(page)
        errors += [f"{lang}.json: no translation for {text!r}" for text in sorted(missing)]
        errors += [f"{lang}.json: unused {text!r}" for text in sorted(set(table) - used)]
        errors += [f"{lang}.json: empty translation for {text!r}" for text, value in table.items() if not value.strip()]
    if errors:
        print("\n".join(errors), file=sys.stderr)
        sys.exit(1)
    print(f"Built {len(PAGES)} pages in {', '.join(LANGS)}")


if __name__ == "__main__":
    main()
