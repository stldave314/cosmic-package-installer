// SPDX-License-Identifier: GPL-3.0

//! RPM package (`.rpm`) support.
//!
//! **Not yet implemented.** Availability detection below is real, so the
//! application correctly reports whether this system could handle an `.rpm` at
//! all, but inspection and installation are not written yet.
//!
//! The intended shape mirrors [`super::deb`]: `rpm -qp --queryformat` for the
//! header fields, `rpm -qlp` for the payload, `rpm -qpR` for requirements, and
//! `dnf install --assumeno` (or `zypper --dry-run`) for the resolved install
//! set. Installation goes through the same [`super::privileged`] dispatcher,
//! which already covers `.rpm` on the PackageKit path without further work.

use std::path::Path;

use super::{
    exec, Action, Availability, Backend, InstalledState, OperationPlan, PackageDetails,
    PackageFormat, Progress, Result,
};

/// Reading an `.rpm` needs only `rpm` itself.
const INSPECT_TOOL: &str = "rpm";

/// Any one of these can resolve dependencies and install; which is present
/// depends on the distribution.
const RESOLVER_TOOLS: &[&str] = &["dnf", "yum", "zypper"];

#[derive(Debug, Default)]
pub struct RpmBackend;

impl RpmBackend {
    pub fn new() -> Self {
        Self
    }
}

impl Backend for RpmBackend {
    fn availability(&self) -> Availability {
        let mut missing = Vec::new();
        if !exec::have(INSPECT_TOOL) {
            missing.push(INSPECT_TOOL);
        }
        // Any one resolver is enough, so this is only missing when none of
        // them is present.
        if !RESOLVER_TOOLS.iter().any(|tool| exec::have(tool)) {
            missing.push(RESOLVER_TOOLS[0]);
        }
        if missing.is_empty() {
            Availability::Ready
        } else {
            Availability::Missing { tools: missing }
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
        format: PackageFormat::Rpm,
    }
}
