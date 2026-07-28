#!/usr/bin/env python3
"""Write a locale file using the fallback locale's structure.

Translations are supplied as a `key -> value` mapping; the section comments,
blank lines and key ordering all come from `i18n/en/`. Generating rather than
hand-editing is what guarantees a locale cannot end up missing a key or
carrying an orphan from a rename — the structure is copied, not retyped.

Values spanning several lines (Fluent select expressions for plurals) are
written verbatim, so a locale can use whichever CLDR plural categories its
language actually has.

This is a development helper, not part of the build. `tools/check-locales.py`
is what verifies the result.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

FALLBACK_FILE = Path("i18n/en/cosmic_package_installer.ftl")
MESSAGE_RE = re.compile(r"^([a-zA-Z][a-zA-Z0-9_-]*)\s*=")


def write_locale(locale: str, translations: dict[str, str]) -> None:
    lines = FALLBACK_FILE.read_text(encoding="utf-8").splitlines()

    out: list[str] = []
    index = 0
    seen: set[str] = set()

    while index < len(lines):
        line = lines[index]
        match = MESSAGE_RE.match(line)
        if not match:
            out.append(line)
            index += 1
            continue

        key = match.group(1)
        seen.add(key)
        if key not in translations:
            raise SystemExit(f"{locale}: no translation supplied for '{key}'")

        value = translations[key]
        if "\n" in value:
            out.append(f"{key} = {value.splitlines()[0]}")
            out.extend(value.splitlines()[1:])
        else:
            out.append(f"{key} = {value}")

        # Skip the fallback's continuation lines; the translation replaced them.
        index += 1
        while index < len(lines) and lines[index][:1].isspace() and lines[index].strip():
            index += 1

    extra = set(translations) - seen
    if extra:
        raise SystemExit(f"{locale}: translations supplied for unknown keys: {sorted(extra)}")

    target = Path("i18n") / locale / "cosmic_package_installer.ftl"
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text("\n".join(out) + "\n", encoding="utf-8")
    print(f"wrote {target} ({len(seen)} keys)")


def main() -> int:
    print(__doc__)
    print("Import write_locale() from a translation script rather than running this directly.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
