# COSMIC Package Installer

A package installer for the COSMIC desktop, in the spirit of GDebi: open a
package file you have downloaded, see exactly what is in it, and install it.

It shows the application's own icon and name, the version and whether that
version is already on your system, the full metadata, the list of files the
package installs, and every dependency marked according to whether it is
already installed, available from your package manager, or missing entirely.
Before you commit to anything it also asks the package manager what would
*really* happen — including packages pulled in indirectly — and shows you the
download and disk-space figures.

The window follows the COSMIC Store's application page, with a collapsible
vertical sidebar for the sections as in COSMIC Settings; the menu bar follows
COSMIC Files. None of it is novel on purpose: this is usually opened by
double-clicking a file you just downloaded, which is not a moment for learning
a new interface.

The sidebar collapses to an overlay by itself once the window is narrower than
about 650 pixels, and the button at the left of the header toggles it at any
width.

## Screenshots

### Details

The icon, name and version as the package itself declares them, whether that
version is already installed, and the full control metadata.

![The Details section, showing a package's icon, name, version, installed
state and metadata](docs/screenshots/details.png)

### Dependencies

What installing would really do — the resolved plan with its disk-space figure
— followed by every declared dependency and its status on this system.

![The Dependencies section, showing the resolved install plan and each
dependency's status](docs/screenshots/dependencies.png)

### Files

Every file in the package payload, with its size.

![The Files section, listing the files the package installs with their
sizes](docs/screenshots/files.png)

## Status

| Format | State |
| --- | --- |
| `.deb` | **Supported.** Inspection, dependency status, install plan, and install / upgrade / downgrade / reinstall / uninstall. |
| `.rpm` | Not implemented yet. The application detects whether this system *could* handle one and says so. |
| `.flatpak` | Not implemented yet. Same detection. |
| `.appimage` | Not implemented yet. Same detection. |

The **View → Supported formats** panel reports what the machine you are running
on can actually handle, so an unimplemented or unavailable format is stated
plainly rather than failing when you press Install.

## Features

- **Application icon and name from the package itself.** The icon is extracted
  from the package payload and the display name is read from its desktop entry,
  so you see "Now Playing" rather than `cosmic-media-now-playing-applet`.
- **Installed-state detection.** Distinguishes not installed, the same version,
  an older version (an upgrade) and a newer version (a downgrade), and offers
  the matching action. Version comparison is delegated to `dpkg`, which is the
  only thing that gets Debian epochs and tildes right.
- **Dependency list with real status.** Every `Depends`, `Pre-Depends`,
  `Recommends`, `Suggests`, `Conflicts`, `Breaks`, `Replaces` and `Provides`
  entry, with alternatives (`exim4 | mail-transport-agent`) resolved as a set.
  Each is marked installed, available, provided by another package (for virtual
  packages), or missing.
- **The resolved install plan.** What the declared dependency list cannot tell
  you is what will *actually* be installed. A single `apt-get --simulate` run
  answers that, including transitive pulls, with download and disk-space totals
  computed from package metadata.
- **File list**, with sizes and symlink targets.
- **Full metadata**, including any control fields not shown elsewhere.
- **Localized** into 11 languages.

## Privileged operations

Nothing in this application runs as root.

Install, upgrade and uninstall go through **PackageKit** when its daemon is
available: it owns the polkit integration, so you get the desktop's own
authentication dialog, and the work happens in a system daemon.

Where PackageKit is absent or disabled, the fallback drives `apt-get` under
`pkexec`. No polkit policy of our own is installed — `pkexec` is invoked on
`apt-get` directly, so polkit's standard administrator prompt applies and names
the program that is genuinely about to run.

The transport is chosen **before** an operation starts, never after one fails.
A half-finished package operation can leave dpkg's database mid-transaction,
and silently retrying it through a different mechanism turns a clear error into
an unpredictable one. You can force either transport in Settings.

## Installing

### From a release

Download the `.deb`, `.rpm` or tarball from the
[releases page](https://github.com/stldave314/cosmic-package-installer/releases).

```sh
sudo apt install ./cosmic-package-installer_*_amd64.deb
```

### From source

```sh
git clone https://github.com/stldave314/cosmic-package-installer.git
cd cosmic-package-installer
./install.sh build
sudo ./install.sh install
```

Build dependencies are the usual libcosmic set — a Rust toolchain plus
`libxkbcommon-dev`, `libwayland-dev`, `libudev-dev`, `libinput-dev`,
`libgbm-dev`, `libseat-dev`, `libssl-dev` and `pkg-config`.

At runtime the `.deb` backend needs `dpkg-deb`, `dpkg-query` and `apt-get`,
which any Debian-derived system already has. `packagekit` and `policykit-1` are
recommended rather than required — between them they provide the two privileged
transports, and the application will tell you if neither is usable.

## Usage

Open a package file from your file manager, or:

```sh
cosmic-package-installer ~/Downloads/something.deb
```

### Keyboard shortcuts

| Shortcut | Action |
| --- | --- |
| `Ctrl+O` | Open a package |
| `Ctrl+W` | Close the open package |
| `Ctrl+R` | Re-read the open package |
| `Ctrl+1` / `Ctrl+2` / `Ctrl+3` | Select the Details / Dependencies / Files section |
| `Ctrl+,` | Settings |
| `Ctrl+Q` | Quit |

### Settings

| Setting | Default | Effect |
| --- | --- | --- |
| Theme | Match desktop | Light, dark, or follow the desktop setting. |
| Install using | PackageKit, or system tools | Which transport carries out privileged operations. The other two options force one or the other. |
| Show recommended packages | On | Include `Recommends` in the dependency list. apt installs these by default, so leaving it on keeps the list honest about what an install pulls in. |
| Show suggested packages | Off | Include `Suggests`. These are never installed and mostly make the list longer. |
| Show the file list | On | Read and display the package payload. Turning it off skips reading the payload index, which is noticeably faster for very large packages. |

## Building packages

```sh
./install.sh deb        # .deb into dist/
./install.sh rpm        # .rpm into dist/
./install.sh tarball    # portable tarball into dist/
./install.sh all        # all three
./install.sh check      # cargo check, clippy, tests, locale consistency
./install.sh hooks      # enable the repository's git hooks
```

Releases are tag-triggered: pushing a `v*` tag runs the same `install.sh all`
that a local build runs and attaches the results to the GitHub Release.

## Development

### Versioning

The version lives in `Cargo.toml` and nowhere else is it written by hand: it is
mirrored into `Cargo.lock` and into the AppStream release history in
`resources/app.metainfo.xml`, and both are updated by the same tool that raises
it.

Commits follow [Conventional Commits](https://www.conventionalcommits.org), and
only the two types that change what the software does move the version:

| Commit | Bump |
| --- | --- |
| `feat:` | minor |
| `fix:` | patch |
| `feat!:`, or a `BREAKING CHANGE:` trailer | major |
| `docs:`, `chore:`, `build:`, `refactor:`, `test:`, `ci:` | none |

```sh
./install.sh hooks
```

points git at `.githooks`, whose `post-commit` reads the message, bumps the
version if the type calls for it, and amends the same commit so the bump and
the change that caused it are never separate commits. It runs post-commit
because the decision needs the commit message, which does not exist yet when
`pre-commit` runs.

To bump without committing:

```sh
python3 tools/bump-version.py minor --note "feat: read Flatpak refs"
```

Releases are cut by tagging the version that is already in `Cargo.toml`:

```sh
git tag v0.1.0 && git push origin v0.1.0
```

### Diagnostic logging

Logging is compiled out by default. To turn it on, set `DEVELOPER_LOGGING` to
`true` in `src/debug.rs` and rebuild; output goes to
`/tmp/cosmic-package-installer.log`, truncated once per launch, with each line
tagged by category (`deb`, `exec`, `pk`, `ops`, `icon`, `ui`, …) so a run can be
filtered with `grep`.

Every packaging target passes the `release-build` feature, which forces the
switch off at compile time regardless of what `DEVELOPER_LOGGING` says, so a
release cannot ship with logging left on. `install.sh` verifies this on the
built binary rather than assuming it — it checks that the log path string is
absent from the release artefact.

### Translations

Locale files live in `i18n/<locale>/cosmic_package_installer.ftl`, with `en` as
the fallback. A missing key falls back to English silently at runtime rather
than failing the build, so any change to a translatable string has to be
applied to every locale in the same change.

```sh
python3 tools/check-locales.py
```

This checks that every locale has exactly the fallback's key set — no missing
keys, no orphans left by a rename, no duplicates — that every argument
placeholder survived translation, and that the fallback and the source agree:
every `fl!("key")` in `src/` exists, and every key defined is actually used.

`tools/gen_locale.py` writes a locale file using the fallback's structure, so a
new or regenerated locale cannot end up with the wrong key set.

## Licence

GPL-3.0. See [LICENSE](LICENSE).
