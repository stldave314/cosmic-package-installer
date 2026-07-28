#!/usr/bin/env python3
"""Raise the project version and everything that has to agree with it.

The version lives in Cargo.toml, is mirrored in Cargo.lock, and is listed again
in the AppStream metainfo's release history. Bumping by hand means three edits
that can drift apart, so this does all three at once and refuses to run if any
of them is not where it expects.

    tools/bump-version.py patch --note "fix: stop truncating long file names"

Called by the post-commit hook, which decides the bump from the commit type;
run it directly to bump without a commit.
"""

import argparse
import datetime
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CARGO_TOML = ROOT / "Cargo.toml"
CARGO_LOCK = ROOT / "Cargo.lock"
METAINFO = ROOT / "resources" / "app.metainfo.xml"

PACKAGE = "cosmic-package-installer"


def die(message):
    sys.exit(f"bump-version: {message}")


def current_version(text):
    match = re.search(r'^version\s*=\s*"([^"]+)"', text, re.MULTILINE)
    if not match:
        die(f"no version field in {CARGO_TOML}")
    return match.group(1)


def next_version(version, bump):
    parts = version.split(".")
    if len(parts) != 3 or not all(part.isdigit() for part in parts):
        die(f"version {version!r} is not major.minor.patch")
    major, minor, patch = (int(part) for part in parts)
    if bump == "major":
        return f"{major + 1}.0.0"
    if bump == "minor":
        return f"{major}.{minor + 1}.0"
    return f"{major}.{minor}.{patch + 1}"


def bump_cargo_toml(new):
    text = CARGO_TOML.read_text()
    # Only the first `version` field — the one in [package]. The dependency
    # tables further down have their own.
    text, count = re.subn(
        r'^version\s*=\s*"[^"]+"', f'version = "{new}"', text, count=1, flags=re.MULTILINE
    )
    if count != 1:
        die("failed to rewrite the version in Cargo.toml")
    CARGO_TOML.write_text(text)


def bump_cargo_lock(new):
    """Rewrite our own entry in the lock file.

    Done textually rather than by running cargo: the hook has to work offline
    and must not have the side effect of updating anything else in the lock.
    """
    if not CARGO_LOCK.exists():
        return
    text = CARGO_LOCK.read_text()
    pattern = re.compile(
        r'(\[\[package\]\]\nname = "%s"\nversion = ")[^"]+(")' % re.escape(PACKAGE)
    )
    text, count = pattern.subn(rf"\g<1>{new}\g<2>", text, count=1)
    if count != 1:
        die("failed to rewrite the version in Cargo.lock")
    CARGO_LOCK.write_text(text)


def add_metainfo_release(new, note, date):
    """Prepend a release entry, so the release history cannot fall behind.

    AppStream lists releases newest first, and a missing entry means software
    centres show the new version with the old version's notes.
    """
    text = METAINFO.read_text()
    if f'<release version="{new}"' in text:
        return
    entry = (
        f'    <release version="{new}" date="{date}">\n'
        "      <description>\n"
        f"        <p>{note}</p>\n"
        "      </description>\n"
        "    </release>\n"
    )
    text, count = re.subn(r"( *<releases>\n)", rf"\g<1>{entry}", text, count=1)
    if count != 1:
        die("no <releases> section in the metainfo file")
    METAINFO.write_text(text)


def escape(text):
    return text.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("bump", choices=["major", "minor", "patch"])
    parser.add_argument(
        "--note",
        default="",
        help="release note for the metainfo entry; defaults to the version alone",
    )
    parser.add_argument(
        "--date", default=None, help="release date (YYYY-MM-DD), defaults to today"
    )
    args = parser.parse_args()

    old = current_version(CARGO_TOML.read_text())
    new = next_version(old, args.bump)
    date = args.date or datetime.date.today().isoformat()
    note = escape(args.note.strip()) or f"Version {new}."

    bump_cargo_toml(new)
    bump_cargo_lock(new)
    add_metainfo_release(new, note, date)

    print(new)


if __name__ == "__main__":
    main()
