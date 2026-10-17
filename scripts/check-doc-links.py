#!/usr/bin/env python3
"""Check relative Markdown links and #anchors across the repository (roadmap M0.3).

Every *.md file that git does not ignore (tracked or untracked) is checked except the frozen
archive (docs/archive/). Inline links, with or without a title or <angle brackets>, and
reference-style definitions (`[label]: path`) are checked. For each relative link, the target file must exist, and a `#fragment` into a Markdown file must match a heading
slug (GitHub style: lowercase, punctuation stripped, spaces to '-', duplicates suffixed -1, -2...)
or an explicit <a name|id="...">. Links inside code fences and inline code are ignored, as are
absolute URLs (scheme:...). Standard library only.

Exit status: 0 if all links resolve, 1 otherwise.
"""
from __future__ import annotations

import os
import re
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
EXCLUDE_PREFIXES = ("docs/archive/",)

LINK = re.compile(r"\]\(\s*<?([^)\s>]+)>?(?:\s+(?:\"[^\"]*\"|'[^']*'|\([^)]*\)))?\s*\)")
REF_DEF = re.compile(r"^ {0,3}\[[^\]]+\]:\s*<?([^\s>]+)>?")
HEADING = re.compile(r"^(#{1,6})\s+(.*?)\s*#*\s*$")
HTML_ANCHOR = re.compile(r'<a\s+(?:name|id)="([^"]+)"')
INLINE_CODE = re.compile(r"`[^`]*`")
MD_LINK_TEXT = re.compile(r"\[([^\]]*)\]\([^)]*\)")


def markdown_files() -> list[str]:
    out = subprocess.run(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard", "*.md"],
        cwd=ROOT, check=True, capture_output=True, text=True,
    ).stdout.split()
    # A tracked file deleted from the working tree is not a link source.
    return sorted(
        f for f in out
        if not f.startswith(EXCLUDE_PREFIXES) and os.path.exists(os.path.join(ROOT, f))
    )


def slug(text: str) -> str:
    text = text.strip().lower()
    text = re.sub(r"[^\w\- ]", "", text)
    return text.replace(" ", "-")


def lines_outside_code(path: str):
    in_fence = False
    with open(path, encoding="utf-8") as fh:
        for number, line in enumerate(fh, 1):
            if line.lstrip().startswith("```"):
                in_fence = not in_fence
                continue
            if not in_fence:
                yield number, line


_anchor_cache: dict[str, set[str]] = {}


def anchors(path: str) -> set[str]:
    if path in _anchor_cache:
        return _anchor_cache[path]
    found: set[str] = set()
    seen: dict[str, int] = {}
    for _, line in lines_outside_code(path):
        m = HEADING.match(line)
        if m:
            text = MD_LINK_TEXT.sub(r"\1", m.group(2)).replace("`", "").replace("*", "")
            base = slug(text)
            if base in seen:
                seen[base] += 1
                found.add(f"{base}-{seen[base]}")
            else:
                seen[base] = 0
                found.add(base)
        found.update(HTML_ANCHOR.findall(line))
    _anchor_cache[path] = found
    return found


def main() -> int:
    total = bad = 0
    for rel in markdown_files():
        src = os.path.join(ROOT, rel)
        for number, line in lines_outside_code(src):
            text = INLINE_CODE.sub("", line)
            urls = [m.group(1) for m in LINK.finditer(text)]
            ref = REF_DEF.match(text)
            if ref:
                urls.append(ref.group(1))
            for url in urls:
                if re.match(r"^[a-zA-Z][a-zA-Z0-9+.-]*:", url):
                    continue  # absolute URL (https:, mailto:, ...)
                total += 1
                target, _, fragment = url.partition("#")
                dest = os.path.normpath(os.path.join(os.path.dirname(src), target)) if target else src
                if not os.path.exists(dest):
                    bad += 1
                    print(f"{rel}:{number}: missing file: {url}")
                elif fragment and dest.endswith(".md") and fragment not in anchors(dest):
                    bad += 1
                    print(f"{rel}:{number}: missing anchor: {url}")
    print(f"checked {total} relative links: {bad} broken")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
