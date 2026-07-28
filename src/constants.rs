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
/// most preferred first. Limited to what the toolkit can actually render — an
/// `.xpm` found in `pixmaps` is no better than no icon at all.
pub const ICON_EXTENSIONS: &[&str] = &["svg", "png"];

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

/// Environment forced on every external command so its output is parseable.
/// Without this, apt and dpkg translate their output and the parser breaks in
/// exactly the locales it was never tested in.
pub const C_LOCALE: [(&str, &str); 2] = [("LC_ALL", "C"), ("LANG", "C")];
