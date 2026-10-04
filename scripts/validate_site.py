#!/usr/bin/env python3
"""Dependency-free checks for the static Titan Pages site."""
from __future__ import annotations

import re
import sys
from html.parser import HTMLParser
from pathlib import Path
from urllib.parse import unquote

ROOT = Path(__file__).resolve().parents[1]
SITE = ROOT / "site"
INDEX = SITE / "index.html"
VOID = {"area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source", "track", "wbr"}
REPO_FILE_PREFIX = "https://github.com/alexsndersoto04-source/aio/blob/main/"


class SiteCheck(HTMLParser):
    def __init__(self) -> None:
        super().__init__(convert_charrefs=True)
        self.ids: set[str] = set()
        self.fragments: list[tuple[str, int]] = []
        self.local_files: list[tuple[str, int]] = []
        self.external: list[tuple[str, int, dict[str, str]]] = []
        self.stack: list[str] = []
        self.errors: list[str] = []
        self.line = 1
        self.lang = ""
        self.title = ""
        self.title_depth = 0
        self.description = ""
        self.i18n_keys: set[str] = set()
        self.i18n_aria_keys: set[str] = set()
        self.image_count = 0

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        attr = {name: value or "" for name, value in attrs}
        if tag == "html":
            self.lang = attr.get("lang", "")
        if tag == "title":
            self.title_depth += 1
        if tag == "meta" and attr.get("name", "").lower() == "description":
            self.description = attr.get("content", "")
        if tag == "img":
            self.image_count += 1
            if "alt" not in attr:
                self.errors.append(f"line {self.getpos()[0]}: img is missing alt")
        if "id" in attr:
            ident = attr["id"]
            if ident in self.ids:
                self.errors.append(f"line {self.getpos()[0]}: duplicate id #{ident}")
            self.ids.add(ident)
        if "data-i18n" in attr:
            self.i18n_keys.add(attr["data-i18n"])
        if "data-i18n-aria" in attr:
            self.i18n_aria_keys.add(attr["data-i18n-aria"])

        line = self.getpos()[0]
        for name in ("href", "src"):
            value = attr.get(name, "")
            if not value:
                continue
            if value.startswith("#"):
                self.fragments.append((value[1:], line))
            elif value.startswith("./"):
                self.local_files.append((value[2:], line))
            elif value.startswith("https://"):
                self.external.append((value, line, attr))
            elif value.startswith(("mailto:", "tel:")):
                continue
            else:
                self.errors.append(f"line {line}: unsupported/non-HTTPS link {value!r}")

        if attr.get("target") == "_blank":
            rel = set(attr.get("rel", "").lower().split())
            if not {"noopener", "noreferrer"}.issubset(rel):
                self.errors.append(f"line {line}: target=_blank must use rel=noopener noreferrer")

        if tag not in VOID:
            self.stack.append(tag)

    def handle_endtag(self, tag: str) -> None:
        if tag == "title" and self.title_depth:
            self.title_depth -= 1
        if tag in VOID:
            return
        if not self.stack:
            self.errors.append(f"line {self.getpos()[0]}: unexpected closing tag </{tag}>")
        elif self.stack[-1] == tag:
            self.stack.pop()
        elif tag in self.stack:
            self.errors.append(f"line {self.getpos()[0]}: mismatched closing tag </{tag}>; expected </{self.stack[-1]}>")
            self.stack.remove(tag)
        else:
            self.errors.append(f"line {self.getpos()[0]}: unexpected closing tag </{tag}>")

    def handle_data(self, data: str) -> None:
        if self.title_depth:
            self.title += data


def main() -> int:
    errors: list[str] = []
    if not INDEX.is_file():
        print("ERROR: site/index.html is missing", file=sys.stderr)
        return 1

    html = INDEX.read_text(encoding="utf-8")
    parser = SiteCheck()
    parser.feed(html)
    parser.close()
    errors.extend(parser.errors)

    if parser.stack:
        errors.append("unclosed HTML elements: " + " > ".join(parser.stack[-8:]))
    if parser.lang != "es":
        errors.append(f"default document language should be es, found {parser.lang!r}")
    if not parser.title.strip():
        errors.append("page title is empty")
    if len(parser.description.strip()) < 60:
        errors.append("meta description is missing or too short")

    for fragment, line in parser.fragments:
        if fragment not in parser.ids:
            errors.append(f"line {line}: anchor #{fragment} has no matching id")

    for relative, line in parser.local_files:
        path = (SITE / unquote(relative)).resolve()
        try:
            path.relative_to(SITE.resolve())
        except ValueError:
            errors.append(f"line {line}: local asset escapes site/: {relative!r}")
            continue
        if not path.is_file():
            errors.append(f"line {line}: local asset does not exist: {relative!r}")

    js_path = SITE / "assets" / "site.js"
    css_path = SITE / "assets" / "site.css"
    for path in (js_path, css_path, SITE / "assets" / "titan-mark.svg", SITE / "robots.txt", SITE / "sitemap.xml"):
        if not path.is_file():
            errors.append(f"required site file is missing: {path.relative_to(ROOT)}")

    js = js_path.read_text(encoding="utf-8") if js_path.is_file() else ""
    translation_keys = set(re.findall(r'^\s*"([A-Za-z][A-Za-z0-9_.-]*)"\s*:', js, flags=re.MULTILINE))
    translation_keys.update(re.findall(r'^\s*([A-Za-z][A-Za-z0-9_.-]*)\s*:', js, flags=re.MULTILINE))
    missing = sorted((parser.i18n_keys | parser.i18n_aria_keys) - translation_keys)
    if missing:
        errors.append("missing ES/EN translation keys: " + ", ".join(missing))

    for url, line, _attrs in parser.external:
        if url.startswith(REPO_FILE_PREFIX):
            relative = unquote(url[len(REPO_FILE_PREFIX):]).split("#", 1)[0]
            if relative and not (ROOT / relative).is_file():
                errors.append(f"line {line}: linked repository file does not exist: {relative}")

    for forbidden in ("localhost", "127.0.0.1", "example.com", "TODO", "FIXME"):
        if re.search(r"\b" + re.escape(forbidden) + r"\b", html, flags=re.IGNORECASE):
            errors.append(f"placeholder/local-only string appears in site HTML: {forbidden}")

    if errors:
        for error in errors:
            print(f"ERROR: {error}", file=sys.stderr)
        return 1

    print(f"OK: {parser.title.strip()}")
    print(f"Checked {len(parser.ids)} unique IDs, {len(parser.fragments)} internal anchors, "
          f"{len(parser.local_files)} local assets, {len(parser.external)} HTTPS links, "
          f"and {len(parser.i18n_keys)} translatable labels.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
