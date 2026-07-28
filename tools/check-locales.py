#!/usr/bin/env python3
"""Verify the Fluent locale files against each other and against the source.

Locale files fail silently: a missing key falls back to English at runtime
rather than failing the build, and a mangled `{ $placeholder }` only shows up
for users of that one language. This checks, for every locale:

  * it has exactly the same key set as the fallback language — no missing keys,
    no orphans left behind by a rename, no duplicates within a file;
  * every key's argument placeholders survived translation intact.

It also checks the fallback locale against the code: every `fl!("key")` in
`src/` must exist, and every key in the fallback must be used by something.

Run from the repository root:

    python3 tools/check-locales.py
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

FALLBACK = "en"
I18N_DIR = Path("i18n")
SRC_DIR = Path("src")

# A message line: `key = value`, at column zero. Continuation lines are indented
# and belong to the preceding key.
MESSAGE_RE = re.compile(r"^([a-zA-Z][a-zA-Z0-9_-]*)\s*=(.*)$")
# `{ $name }`, with any amount of internal whitespace.
PLACEHOLDER_RE = re.compile(r"\{\s*\$([a-zA-Z][a-zA-Z0-9_-]*)\s*\}")
# `fl!("key")` or `fl!("key", arg = ...)`.
FL_RE = re.compile(r'\bfl!\s*\(\s*"([^"]+)"')


def parse_locale(path: Path) -> tuple[dict[str, set[str]], list[str]]:
    """Return each key's placeholder names, plus any duplicate keys found."""
    entries: dict[str, set[str]] = {}
    duplicates: list[str] = []
    current: str | None = None

    for raw in path.read_text(encoding="utf-8").splitlines():
        if not raw.strip() or raw.lstrip().startswith("#"):
            continue
        if raw[0].isspace():
            # Continuation of the previous message, including select
            # expressions, whose placeholders count too.
            if current is not None:
                entries[current] |= set(PLACEHOLDER_RE.findall(raw))
            continue
        match = MESSAGE_RE.match(raw)
        if not match:
            continue
        key, value = match.group(1), match.group(2)
        if key in entries:
            duplicates.append(key)
        entries.setdefault(key, set())
        entries[key] |= set(PLACEHOLDER_RE.findall(value))
        current = key

    return entries, duplicates


def keys_used_in_source() -> set[str]:
    keys: set[str] = set()
    for path in SRC_DIR.rglob("*.rs"):
        keys |= set(FL_RE.findall(path.read_text(encoding="utf-8")))
    return keys


def main() -> int:
    locales = sorted(directory.name for directory in I18N_DIR.iterdir() if directory.is_dir())
    if FALLBACK not in locales:
        print(f"error: no fallback locale '{FALLBACK}' in {I18N_DIR}/")
        return 1

    def locale_file(name: str) -> Path:
        candidates = sorted(( I18N_DIR / name).glob("*.ftl"))
        if len(candidates) != 1:
            raise SystemExit(f"error: expected exactly one .ftl in {I18N_DIR / name}")
        return candidates[0]

    fallback_entries, fallback_duplicates = parse_locale(locale_file(FALLBACK))
    problems: list[str] = []

    for key in sorted(fallback_duplicates):
        problems.append(f"{FALLBACK}: duplicate key '{key}'")

    # The fallback locale against the code.
    used = keys_used_in_source()
    for key in sorted(used - set(fallback_entries)):
        problems.append(f"{FALLBACK}: key '{key}' is used in src/ but not defined")
    for key in sorted(set(fallback_entries) - used):
        problems.append(f"{FALLBACK}: key '{key}' is defined but never used in src/")

    # Every other locale against the fallback.
    for name in locales:
        if name == FALLBACK:
            continue
        entries, duplicates = parse_locale(locale_file(name))

        for key in sorted(duplicates):
            problems.append(f"{name}: duplicate key '{key}'")
        for key in sorted(set(fallback_entries) - set(entries)):
            problems.append(f"{name}: missing key '{key}'")
        for key in sorted(set(entries) - set(fallback_entries)):
            problems.append(f"{name}: orphaned key '{key}'")

        for key in sorted(set(fallback_entries) & set(entries)):
            expected, actual = fallback_entries[key], entries[key]
            if expected != actual:
                missing = ", ".join(sorted(expected - actual)) or "none"
                extra = ", ".join(sorted(actual - expected)) or "none"
                problems.append(
                    f"{name}: key '{key}' placeholder mismatch "
                    f"(missing: {missing}; unexpected: {extra})"
                )

    if problems:
        for problem in problems:
            print(problem)
        print(f"\n{len(problems)} problem(s) across {len(locales)} locale(s)")
        return 1

    print(f"{len(locales)} locales, {len(fallback_entries)} keys, all consistent")
    return 0


if __name__ == "__main__":
    sys.exit(main())
