// SPDX-License-Identifier: GPL-3.0

//! Persistent user settings, stored and loaded via `cosmic-config`.
//!
//! Only things a user should reasonably want to change live here. Values that
//! exist to tune the implementation belong in [`crate::constants`] instead.

use cosmic::{
    cosmic_config::{self, cosmic_config_derive::CosmicConfigEntry, CosmicConfigEntry},
    theme,
};
use serde::{Deserialize, Serialize};

/// Bumped when a field is removed or its meaning changes, so `cosmic-config`
/// discards an incompatible stored config rather than mis-reading it.
pub const CONFIG_VERSION: u64 = 1;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum AppTheme {
    Dark,
    Light,
    #[default]
    System,
}

impl AppTheme {
    pub fn theme(&self) -> theme::Theme {
        match self {
            Self::Dark => theme::Theme::dark(),
            Self::Light => theme::Theme::light(),
            Self::System => theme::system_preference(),
        }
    }
}

/// Which transport is used for install, upgrade and remove.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum PrivilegeBackend {
    /// Use PackageKit when its daemon answers, otherwise fall back to the
    /// native package tools under `pkexec`.
    #[default]
    Auto,
    /// Always use PackageKit; report an error if it is unreachable.
    PackageKit,
    /// Always drive the native tools under `pkexec`, even if PackageKit is up.
    /// Useful when the PackageKit backend for the distribution is unreliable.
    Native,
}

/// Which Flatpak installation an install goes to.
///
/// This is a user setting rather than a tuning value because the two are
/// genuinely different choices with different consequences, and neither is
/// right for everyone: a user install needs no password and is visible only to
/// the person who made it, while a system install needs administrator rights
/// and serves every account on the machine.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum FlatpakScope {
    /// Install under `~/.local/share/flatpak`. Needs no privileges at all,
    /// which is why it is the default: the common case for opening a downloaded
    /// `.flatpak` is one person installing something for themselves, and asking
    /// for a password to do it would be asking for one that is not needed.
    #[default]
    User,
    /// Install for every user. Authorised by Flatpak's own polkit actions.
    System,
}

#[derive(Clone, CosmicConfigEntry, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[version = 1]
pub struct Config {
    pub app_theme: AppTheme,
    /// How privileged operations are performed.
    pub privilege_backend: PrivilegeBackend,
    /// Where Flatpaks are installed.
    pub flatpak_scope: FlatpakScope,
    /// Include `Recommends` alongside `Depends` in the dependency list.
    ///
    /// apt installs recommended packages by default, so leaving this on keeps
    /// the displayed list honest about what an install will actually pull in.
    pub show_recommends: bool,
    /// Include `Suggests` in the dependency list. Off by default: suggestions
    /// are not installed and usually just make the list longer.
    pub show_suggests: bool,
    /// Show the full file list. Turning this off skips reading the payload
    /// index, which is noticeably faster for very large packages.
    pub show_file_list: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            app_theme: AppTheme::System,
            privilege_backend: PrivilegeBackend::Auto,
            flatpak_scope: FlatpakScope::User,
            show_recommends: true,
            show_suggests: false,
            show_file_list: true,
        }
    }
}
