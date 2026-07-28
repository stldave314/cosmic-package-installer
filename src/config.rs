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

#[derive(Clone, CosmicConfigEntry, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[version = 1]
pub struct Config {
    pub app_theme: AppTheme,
    /// How privileged operations are performed.
    pub privilege_backend: PrivilegeBackend,
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
            show_recommends: true,
            show_suggests: false,
            show_file_list: true,
        }
    }
}
