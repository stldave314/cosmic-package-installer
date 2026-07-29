// SPDX-License-Identifier: GPL-3.0

//! Compile-time tuning values, gathered in one place.
//!
//! These are *implementation* tuning knobs, not user settings — anything the
//! user should be able to change lives in [`crate::config`] and is persisted via
//! `cosmic-config`. Keeping these compile-time avoids a second configuration
//! mechanism, a startup file read, and a class of "malformed config" failures,
//! while still giving one obvious place to find and adjust them.

use std::time::Duration;

// ── Identity ────────────────────────────────────────────────────────────────

/// D-Bus / desktop-entry identifier. Must match the `.desktop` file name
/// installed by the packaging targets, or the window will not pick up its icon.
pub const APP_ID: &str = "com.github.cosmic_package_installer";

/// Icon name shipped alongside the desktop entry. Matches [`APP_ID`] because
/// the packaging targets install the icon under that name.
pub const APP_ICON: &str = APP_ID;

/// Consulted by the About dialog, derived from the `repository` field in
/// Cargo.toml so the URL has a single source of truth.
pub const REPOSITORY_URL: &str = env!("CARGO_PKG_REPOSITORY");

/// Where users are sent to report a problem.
pub const ISSUES_URL: &str = concat!(env!("CARGO_PKG_REPOSITORY"), "/issues");

// ── Window ──────────────────────────────────────────────────────────────────

/// Initial window size. Tall enough that the metadata list and the first few
/// dependencies are visible without scrolling.
pub const WINDOW_WIDTH: f32 = 800.0;
pub const WINDOW_HEIGHT: f32 = 700.0;

/// Below this the two-column metadata grid is folded into a single column.
pub const WINDOW_MIN_WIDTH: f32 = 420.0;
pub const WINDOW_MIN_HEIGHT: f32 = 400.0;

// ── Layout ──────────────────────────────────────────────────────────────────

/// Size in pixels of the package icon in the header.
pub const ICON_SIZE_HEADER: u16 = 96;

/// Size in pixels of the small icons used in dependency and file rows.
pub const ICON_SIZE_ROW: u16 = 16;

/// Content is centred and capped at this width so the metadata and dependency
/// lists don't stretch into unreadably long lines on a maximised window.
pub const MAX_CONTENT_WIDTH: f32 = 900.0;

// ── Package inspection ──────────────────────────────────────────────────────

/// Longest a synchronous inspection command (`dpkg-deb --field`, `rpm -qp`,
/// `flatpak info`) may run before it is abandoned. These read a local file and
/// should return promptly; a hang means something is badly wrong and the user
/// is better served by an error than an unresponsive window.
pub const INSPECT_TIMEOUT: Duration = Duration::from_secs(30);

/// Longest a dependency-resolution command (`apt-get --simulate`) may run.
/// More generous than [`INSPECT_TIMEOUT`] because apt may need to consult the
/// package lists on disk, which is slow on a cold cache.
pub const RESOLVE_TIMEOUT: Duration = Duration::from_secs(120);

/// Longest an install/upgrade/remove operation may run before it is abandoned.
/// Deliberately long: the user may be prompted for a password, a large package
/// may need unpacking, and maintainer scripts can take a while.
pub const OPERATION_TIMEOUT: Duration = Duration::from_secs(60 * 60);

/// Above this many entries the file list is truncated in the view, with a note
/// giving the true total. Rendering tens of thousands of rows costs far more
/// than the information is worth.
pub const MAX_FILES_SHOWN: usize = 5_000;

/// Candidate directories inside a `.deb`/`.rpm` payload that are searched for an
/// application icon, in the order they are preferred. `scalable` first because
/// an SVG renders cleanly at any size.
pub const ICON_SEARCH_DIRS: &[&str] = &[
    "usr/share/icons/hicolor/scalable/apps/",
    "usr/share/icons/hicolor/512x512/apps/",
    "usr/share/icons/hicolor/256x256/apps/",
    "usr/share/icons/hicolor/128x128/apps/",
    "usr/share/icons/hicolor/96x96/apps/",
    "usr/share/icons/hicolor/64x64/apps/",
    "usr/share/icons/hicolor/48x48/apps/",
    "usr/share/pixmaps/",
];

/// File extensions accepted when extracting an icon from a package payload,
/// most preferred first. Limited to what can actually be got onto the screen:
/// `svg` and `png` the toolkit decodes itself, and `xpm` via [`crate::backend::xpm`].
///
/// `xpm` is last on purpose. It is a legacy format and the two ahead of it are
/// better in every respect, so it is only reached when a package ships nothing
/// else — which, for the older packages that still put an icon in `pixmaps`, is
/// the difference between an icon and the generic placeholder.
pub const ICON_EXTENSIONS: &[&str] = &["svg", "png", "xpm"];

// ── XPM ─────────────────────────────────────────────────────────────────────
//
// Every bound here exists because the value it limits is read out of the file
// being decoded. An XPM header states its own dimensions and colour count, and
// a file that is malformed — or hostile — can state whatever it likes; without
// these, `65535 65535` in a header is a 17 GB allocation.

/// Largest XPM that will be decoded at all. Comfortably above any real icon:
/// the largest on a typical system is a couple of hundred kilobytes.
pub const XPM_MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;

/// Largest edge accepted, in pixels. An icon beyond this is not an icon, and
/// the square of this bounds the output allocation.
pub const XPM_MAX_DIMENSION: u32 = 1024;

/// Largest colour table accepted. Real icons are well inside this — the biggest
/// found on this machine uses 1770.
pub const XPM_MAX_COLORS: usize = 65_536;

/// Longest per-pixel key accepted. Two is universal in practice; four allows
/// for an unusually large palette without allowing a key long enough to make
/// row parsing expensive.
pub const XPM_MAX_CHARS_PER_PIXEL: usize = 4;

/// Shown in place of the application icon when the package carries none.
pub const FALLBACK_ICON: &str = "package-x-generic";

// ── PackageKit ──────────────────────────────────────────────────────────────

// The daemon's bus name, object path and interface names are not repeated
// here: zbus's `#[proxy]` attribute needs them as literals in the attribute
// itself and cannot reference a constant, so `backend/packagekit.rs` is their
// single source of truth.

/// How long to wait for the daemon to answer a probe before deciding it is not
/// usable and falling back to the native package tools. Short, because this
/// runs on the path to showing the window and the fallback always works.
pub const PK_PROBE_TIMEOUT: Duration = Duration::from_secs(3);

/// PackageKit transaction flag `NONE`. Passed to `InstallFiles`/`RemovePackages`
/// so the daemon performs the operation rather than simulating it.
pub const PK_FLAG_NONE: u64 = 1 << 0;

/// Hints set on every transaction before it starts.
///
/// `interactive=true` is the one that matters: it is how a client tells
/// PackageKit that a person is waiting, which PackageKit passes to polkit as
/// permission to open an authentication dialog. Without it polkit is asked
/// non-interactively, refuses anything needing an administrator, and the
/// transaction fails with "Failed to obtain authentication" having never
/// prompted for anything.
pub const PK_HINTS: &[&str] = &["interactive=true"];

/// `PkErrorEnum` value for a transaction polkit would not authorise.
///
/// Taken from the daemon's own `ErrorCode` signal rather than a header: the
/// development headers are not a runtime dependency, and the value is part of
/// the wire protocol, not of any library this links against.
pub const PK_ERROR_NOT_AUTHORIZED: u32 = 48;

/// How long to keep listening for an `ErrorCode` signal after `Finished`.
///
/// PackageKit emits the two in either order, and the one that says *why* a
/// transaction failed frequently arrives second. Returning the moment
/// `Finished` lands therefore throws away the explanation; this is how long it
/// is worth waiting for one that is already on its way.
pub const PK_ERROR_GRACE: Duration = Duration::from_millis(500);

// ── Native tool fallbacks ───────────────────────────────────────────────────

/// Programs used by the `.deb` backend, looked up on `PATH`. Inspection needs
/// only `dpkg-deb`; the rest are for status and dependency resolution.
pub const DEB_INSPECT_TOOL: &str = "dpkg-deb";
pub const DEB_QUERY_TOOL: &str = "dpkg-query";
pub const DEB_COMPARE_TOOL: &str = "dpkg";
pub const DEB_CACHE_TOOL: &str = "apt-cache";
pub const DEB_APT_TOOL: &str = "apt-get";

/// Program used to obtain administrator privileges when PackageKit is not
/// available. Invoked without a registered polkit action, so it falls back to
/// the standard `org.freedesktop.policykit.exec` prompt.
pub const PKEXEC: &str = "pkexec";

/// Everything the Flatpak backend needs is behind this one program. Note that
/// it is *not* routed through [`PKEXEC`]: a user-scope install needs no
/// privileges at all, and a system-scope one is authorised by Flatpak's own
/// polkit actions inside its system helper.
pub const FLATPAK_TOOL: &str = "flatpak";

/// Refreshes the desktop entry database after an AppImage is integrated or
/// removed. Absent on a minimal system, in which case the new entry simply
/// appears at the next login instead of immediately.
pub const DESKTOP_DATABASE_TOOL: &str = "update-desktop-database";

// ── Flatpak bundle header ───────────────────────────────────────────────────

/// Sanity bound on the number of entries in a bundle's metadata dictionary.
///
/// A real bundle has around ten. The bound exists because the entry count is
/// derived from a length read out of the file itself, and a corrupt or hostile
/// file must not be able to turn that into an unbounded allocation.
pub const BUNDLE_MAX_HEADER_ENTRIES: usize = 4_096;

/// How much of a header entry is read to recover its key.
///
/// Keys are short and sit at the start of the entry, so this is enough to
/// identify one without reading its value — which matters because the
/// compressed payload of the whole bundle is stored as a header entry too, and
/// may be gigabytes.
pub const BUNDLE_KEY_PROBE_BYTES: u64 = 512;

/// Largest header entry whose value is read. Comfortably above the biggest
/// field of interest (an embedded 128×128 icon) and far below the payload
/// entries, which are skipped by size.
pub const BUNDLE_MAX_VALUE_BYTES: u64 = 4 * 1024 * 1024;

// ── AppImage ────────────────────────────────────────────────────────────────

/// The two bytes following the ELF header that identify an AppImage, at
/// [`APPIMAGE_MAGIC_OFFSET`]. The byte after them is the format revision: `1`
/// for the original ISO-9660 layout, `2` for the SquashFS one in use since.
pub const APPIMAGE_MAGIC: [u8; 2] = [0x41, 0x49];
pub const APPIMAGE_MAGIC_OFFSET: u64 = 8;

/// Largest AppImage that will be copied to a temporary directory in order to
/// run it for its metadata.
///
/// The copy is only made when the user has consented to running the file (by
/// pressing Install) and it is not already executable. Beyond this size the
/// operation falls back to what the file name and the ELF header alone can say.
pub const APPIMAGE_MAX_INSPECT_COPY: u64 = 2 * 1024 * 1024 * 1024;

/// Largest file the extraction is allowed to read back — desktop entry,
/// AppStream document or icon.
///
/// Everything read here was written by the AppImage's own runtime out of a
/// SquashFS the file controls, so its size is attacker-chosen. Without a cap a
/// bundle carrying a multi-gigabyte `metainfo.xml` would have it read whole into
/// memory during inspection. Comfortably above any real metadata file.
pub const APPIMAGE_MAX_METADATA_BYTES: u64 = 16 * 1024 * 1024;

/// Directory the AppImage runtime extracts into, relative to the working
/// directory it is run from. Fixed by the runtime, not a choice.
pub const APPIMAGE_EXTRACT_DIR: &str = "squashfs-root";

/// Where an integrated AppImage and its desktop files are placed, relative to
/// the user's home directory.
///
/// All three are under `$HOME` on purpose: integrating an AppImage needs no
/// administrator rights, and taking them anyway would be asking for a password
/// to copy a file the user already owns.
pub const APPIMAGE_INSTALL_DIR: &str = ".local/bin";
pub const APPIMAGE_DESKTOP_DIR: &str = ".local/share/applications";
pub const APPIMAGE_ICON_DIR: &str = ".local/share/icons/hicolor";

/// Desktop-entry keys written into an integrated AppImage's `.desktop` file.
///
/// `X-AppImage-Source` is what makes the entry recognisable as ours on a later
/// run: it records the integrated copy's path, which is how the backend answers
/// "is this already installed" without a package database to ask.
pub const APPIMAGE_KEY_SOURCE: &str = "X-AppImage-Source";
pub const APPIMAGE_KEY_VERSION: &str = "X-AppImage-Version";

/// How often the copy progress of an integration is reported, in bytes.
/// Small enough for a smooth bar, large enough not to flood the UI thread.
pub const APPIMAGE_COPY_CHUNK: usize = 4 * 1024 * 1024;

/// Environment forced on every external command so its output is parseable.
/// Without this, apt and dpkg translate their output and the parser breaks in
/// exactly the locales it was never tested in.
pub const C_LOCALE: [(&str, &str); 2] = [("LC_ALL", "C"), ("LANG", "C")];
