// SPDX-License-Identifier: GPL-3.0

//! Flatpak bundle (`.flatpak`) and reference (`.flatpakref`) support.
//!
//! **Not yet implemented.** Availability detection below is real, so the
//! application correctly reports whether this system can handle a Flatpak at
//! all, but inspection and installation are not written yet.
//!
//! Flatpak differs from the other formats in ways the eventual implementation
//! has to account for rather than paper over:
//!
//! * A bundle carries its own AppStream metadata and icon, so the metadata and
//!   icon come from inside the file rather than from a control header.
//! * Dependencies are runtimes, not packages. The dependency view becomes "this
//!   needs `org.gnome.Platform//47`, which is / is not installed", plus the
//!   remotes it would be pulled from.
//! * Installation can target the user or the system. A user install needs no
//!   privileges at all, so it must not go through [`super::privileged`].

use std::path::Path;

use super::{
    Action, Availability, Backend, InstalledState, OperationPlan, PackageDetails, PackageFormat,
    Progress, Result,
};

/// Everything Flatpak needs is behind its own command-line tool.
const REQUIRED_TOOLS: &[&str] = &["flatpak"];

#[derive(Debug, Default)]
pub struct FlatpakBackend;

impl FlatpakBackend {
    pub fn new() -> Self {
        Self
    }
}

impl Backend for FlatpakBackend {
    fn availability(&self) -> Availability {
        Availability::from_required(REQUIRED_TOOLS)
    }

    fn inspect(&self, _path: &Path, _include_payload: bool) -> Result<PackageDetails> {
        Err(not_implemented())
    }

    fn installed_state(&self, _details: &PackageDetails) -> Result<InstalledState> {
        Err(not_implemented())
    }

    fn resolve_dependencies(&self, _details: &mut PackageDetails) -> Result<()> {
        Err(not_implemented())
    }

    fn plan(&self, _details: &PackageDetails, _action: Action) -> Result<OperationPlan> {
        Err(not_implemented())
    }

    fn perform(
        &self,
        _details: &PackageDetails,
        _action: Action,
        _on_progress: &mut dyn FnMut(Progress),
    ) -> Result<()> {
        Err(not_implemented())
    }
}

fn not_implemented() -> super::Error {
    super::Error::NotImplemented {
        format: PackageFormat::Flatpak,
    }
}
