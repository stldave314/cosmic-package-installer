// SPDX-License-Identifier: GPL-3.0

//! AppImage (`.appimage`) support.
//!
//! **Not yet implemented.** Availability detection below is real, so the
//! application correctly reports whether this system can run an AppImage, but
//! inspection and integration are not written yet.
//!
//! AppImage is the odd one out and the eventual implementation should not
//! pretend otherwise. There is no package manager, no dependency metadata and
//! no package database, so several parts of the model degrade honestly rather
//! than being faked:
//!
//! * "Installing" means copying the file somewhere on `PATH`, marking it
//!   executable, and extracting its desktop entry and icon so the desktop can
//!   find it. None of that needs administrator rights when done under the
//!   user's home directory, so it must not go through [`super::privileged`].
//! * "Is it installed" is answered by looking for a previously integrated copy,
//!   not by querying a database.
//! * Dependencies are bundled by design, so the dependency list will be empty
//!   and the view should say so rather than showing an empty section.
//! * Metadata and icon come from the embedded AppImage payload, read with
//!   `--appimage-extract`, which needs a writable temporary directory.

use std::path::Path;

use super::{
    exec, Action, Availability, Backend, InstalledState, OperationPlan, PackageDetails,
    PackageFormat, Progress, Result,
};

/// AppImages are self-mounting through FUSE. Without it they can still be
/// extracted, but nothing about them will run, so its absence is worth
/// reporting up front rather than at the moment the user tries to launch one.
const FUSE_TOOLS: &[&str] = &["fusermount3", "fusermount"];

#[derive(Debug, Default)]
pub struct AppImageBackend;

impl AppImageBackend {
    pub fn new() -> Self {
        Self
    }
}

impl Backend for AppImageBackend {
    fn availability(&self) -> Availability {
        if FUSE_TOOLS.iter().any(|tool| exec::have(tool)) {
            Availability::Ready
        } else {
            Availability::Missing {
                tools: vec![FUSE_TOOLS[0]],
            }
        }
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
        format: PackageFormat::AppImage,
    }
}
