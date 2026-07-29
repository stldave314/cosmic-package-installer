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

### Other formats

The window is the same whatever you open, and so are the actions: a Flatpak
bundle gets the same Install button as a `.deb`, and an installed one the same
Upgrade and Uninstall pair. Here a bundle's own AppStream metadata and icon,
read straight out of the file:

![A Flatpak bundle open in the Details section, showing its icon, name,
version, licence and homepage](docs/screenshots/flatpak.png)

Its dependency is a runtime rather than a list of packages, checked against
what is actually installed:

![The Dependencies section for a Flatpak bundle, showing the install plan and
the runtime it needs marked as installed](docs/screenshots/flatpak-dependencies.png)

An AppImage — this one's icon is an XPM, which nothing else on the desktop can
draw:

![An AppImage open in the Details section, showing the icon decoded from the
XPM inside it](docs/screenshots/appimage.png)

Its Files section lists what integrating it will actually put in your home
directory, rather than the contents of the image, which are never unpacked:

![The Files section for an AppImage, listing the integrated copy and its
desktop entry](docs/screenshots/appimage-files.png)

## Status

| Format | State |
| --- | --- |
| `.deb` | **Supported.** Inspection, dependency status, install plan, and install / upgrade / downgrade / reinstall / uninstall. |
| `.flatpak`, `.flatpakref` | **Supported.** Inspection, runtime status, and install / upgrade / downgrade / reinstall / uninstall, for either the user or the whole system. No file list — see below. |
| `.appimage` | **Supported.** Inspection and desktop integration: install / upgrade / downgrade / reinstall / uninstall, entirely within your home directory. |
| `.rpm` | Not implemented yet. The application detects whether this system *could* handle one and says so. |

The **View → Supported formats** panel reports what the machine you are running
on can actually handle, so an unimplemented or unavailable format is stated
plainly rather than failing when you press Install.

Two things the other formats provide are genuinely unavailable here, and are
reported as unavailable rather than shown as empty:

- **A Flatpak bundle has no file list.** It carries no index, so the only way to
  enumerate it is to unpack the whole thing. The Files section says so instead
  of showing nothing, which would read as "installs no files".
- **An AppImage has no dependencies.** It bundles them, so the Dependencies
  section says that rather than showing an empty list. Its Files section shows
  what integration will actually place in your home directory — the AppImage
  itself and its desktop entry — not the contents of the image, none of which is
  unpacked anywhere.

## Features

- **Application icon and name from the package itself.** The icon is extracted
  from the package payload and the display name is read from its desktop entry
  or its AppStream data, so you see "Now Playing" rather than
  `cosmic-media-now-playing-applet`. SVG, PNG and XPM icons are all understood —
  XPM via a bounds-checked decoder of our own, since no modern image library
  reads it, which is what gets icons out of older `.deb`s that only ship one in
  `pixmaps` and out of AppImages like VeraCrypt whose only icon is XPM. What a
  file *contains* decides how it is decoded, not what it is called: icon themes
  are full of SVGs behind `.xpm` compatibility names, and an AppImage's
  `.DirIcon` has no extension at all.
- **Installed-state detection.** Distinguishes not installed, the same version,
  an older version (an upgrade) and a newer version (a downgrade), and offers
  the matching action. For `.deb` the version comparison is delegated to `dpkg`,
  which is the only thing that gets Debian epochs and tildes right; Flatpak and
  AppImage have no such authority to defer to and get a SemVer-style comparison
  instead, so `1.0-rc1` correctly sorts before `1.0`.
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
- **Flatpak bundles read without unpacking them.** A `.flatpak` is a single
  GVariant value whose leading dictionary holds the ref, the `metadata` file,
  the AppStream data and the icons — which is where Flatpak itself reads them
  from. Only that dictionary's framing is walked, with seeks, so a 200 MB bundle
  is inspected as quickly as a 10 KB one and its compressed payload is never
  read at all.
- **AppImages integrated into your home directory.** Installing copies the file
  to `~/.local/bin`, marks it executable, and puts its desktop entry and icon
  where the desktop will find them. An AppImage that bundles no desktop entry
  gets a minimal one written for it, so it still appears in the launcher and can
  still be uninstalled from here afterwards. An XPM icon is converted to PNG on
  the way into the icon theme — COSMIC itself cannot render an `.xpm` placed
  there, so installing it unconverted would mean a launcher entry with a hole
  where its icon should be.
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

All of the above applies to `.deb` and `.rpm`. The other two formats need less:

- **Flatpak** is not routed through either transport. A user install writes only
  to `~/.local/share/flatpak` and needs no privileges at all, and a system
  install is authorised by Flatpak's own polkit actions inside its system
  helper — a better prompt than anything this application could raise, because
  it names the operation rather than a shell command. Which of the two an
  install uses is a setting.
- **AppImage** never needs privileges. Everything it writes is under your home
  directory. Reading the metadata inside one means *running* it — that is how
  `--appimage-extract` works, and the file is attacker-supplied code — so the
  installer treats running it as a decision, not a given:
    - **Inspection runs it only if you have already marked it executable.**
      Opening a file to look at it is not consent to run it. A freshly
      downloaded AppImage you have not `chmod +x`'d shows its name, size and
      format from the file itself, and says the rest could not be read without
      running it. Mark it executable, or press Install, to see the full details.
    - **Installing runs it,** because that is the consent inspection lacks; a
      non-executable file is copied to a temporary directory and the *copy* is
      marked, so your own file is never changed.
    - Files the runtime unpacks are only read back if they genuinely stay inside
      the extraction, so a booby-trapped image cannot use a symlink to make the
      installer read files elsewhere on your disk.

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

The `.flatpak` backend needs `flatpak`, and the `.appimage` backend needs FUSE
(`fusermount3`) for the AppImages themselves to run. Neither is required to
start the application: a format whose tools are missing is reported as
unsupported under **View → Supported formats** rather than failing later.

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
| Install using | PackageKit, or system tools | Which transport carries out privileged `.deb` and `.rpm` operations. The other two options force one or the other. |
| Install Flatpaks for | Just me | Whether a Flatpak goes to your own installation or the system-wide one. "Just me" needs no password; "Everyone on this computer" is authorised by Flatpak's own polkit prompt. Changing it re-reads the open package, since whether it counts as installed depends on where you are looking. |
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
tagged by category (`deb`, `fpak`, `aimg`, `exec`, `pk`, `ops`, `icon`, `ui`, …)
so a run can be filtered with `grep`.

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
