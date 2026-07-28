// SPDX-License-Identifier: GPL-3.0

//! Choosing how a privileged operation is carried out.
//!
//! Two transports are available and they are not interchangeable in quality:
//!
//! * **PackageKit** is preferred. It runs the operation in an existing system
//!   daemon, handles polkit authentication itself, and reports structured
//!   progress. Nothing in this application ever runs as root.
//! * **The distribution's own tools under `pkexec`** are the fallback, for
//!   systems where PackageKit is absent, disabled, or has a backend that cannot
//!   install local files.
//!
//! The transport is chosen *before* the operation starts, never after one
//! fails. A failed package operation can leave dpkg's database mid-transaction,
//! and silently retrying it through a different mechanism turns a clear error
//! into an unpredictable one.

use std::sync::{OnceLock, RwLock};

use super::{
    deb,
    exec::{self},
    Action, Error, PackageDetails, Progress, Result,
};
use crate::config::PrivilegeBackend;
use crate::constants::{DEB_APT_TOOL, OPERATION_TIMEOUT, PKEXEC};
use crate::debug::OPS;
use crate::debug_log;

/// The user's configured transport preference.
///
/// Held here rather than passed through [`Backend::perform`](super::Backend)
/// because it is a property of the session, not of the package being operated
/// on, and threading it through every backend signature would put a setting
/// nobody but this module reads into every one of them.
static PREFERENCE: RwLock<PrivilegeBackend> = RwLock::new(PrivilegeBackend::Auto);

/// Result of probing for the PackageKit daemon, cached for the process.
///
/// The probe costs a D-Bus round trip and, if the daemon has to be activated,
/// can take seconds. Doing it once keeps that cost off the path of every
/// button press. A daemon appearing mid-session is not worth re-probing for:
/// the fallback works, and the user can restart the application.
static PACKAGEKIT_AVAILABLE: OnceLock<bool> = OnceLock::new();

/// Record the user's transport preference. Called when the config loads and
/// whenever it changes.
pub fn set_preference(preference: PrivilegeBackend) {
    debug_log!(OPS, "privilege backend preference set to {preference:?}");
    if let Ok(mut guard) = PREFERENCE.write() {
        *guard = preference;
    }
}

fn preference() -> PrivilegeBackend {
    PREFERENCE
        .read()
        .map(|guard| *guard)
        .unwrap_or(PrivilegeBackend::Auto)
}

/// Whether PackageKit is usable, probing at most once.
pub fn packagekit_available() -> bool {
    *PACKAGEKIT_AVAILABLE.get_or_init(super::packagekit::is_available)
}

/// Which transport an operation will actually use.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Transport {
    PackageKit,
    Native,
}

/// Decide the transport for this operation.
pub fn transport() -> Transport {
    match preference() {
        PrivilegeBackend::PackageKit => Transport::PackageKit,
        PrivilegeBackend::Native => Transport::Native,
        PrivilegeBackend::Auto => {
            if packagekit_available() {
                Transport::PackageKit
            } else {
                Transport::Native
            }
        }
    }
}

/// Install, upgrade, downgrade, reinstall or remove a Debian package.
pub fn perform_deb(
    details: &PackageDetails,
    action: Action,
    on_progress: &mut dyn FnMut(Progress),
) -> Result<()> {
    let transport = transport();
    debug_log!(
        OPS,
        "{action:?} {} v{} via {transport:?}",
        details.id,
        details.version
    );

    match transport {
        Transport::PackageKit => {
            if action.is_install() {
                super::packagekit::install_file(&details.path, OPERATION_TIMEOUT, on_progress)
            } else {
                super::packagekit::remove_package(&details.id, OPERATION_TIMEOUT, on_progress)
            }
        }
        Transport::Native => perform_deb_native(details, action, on_progress),
    }
}

/// Drive `apt-get` under `pkexec`.
///
/// `pkexec` is invoked on `apt-get` directly rather than on a wrapper script:
/// with no polkit action registered for it, polkit falls back to its standard
/// administrator prompt, and the dialog then names `/usr/bin/apt-get` — which
/// is exactly what is about to run. A wrapper would show the user the name of
/// something they have no way to verify.
///
/// `pkexec` also resets the environment to a minimal, sanitised set. That has a
/// useful side effect here: with no locale variables inherited, `apt-get` writes
/// its output in the C locale, which is what the progress parser expects.
fn perform_deb_native(
    details: &PackageDetails,
    action: Action,
    on_progress: &mut dyn FnMut(Progress),
) -> Result<()> {
    if !exec::have(PKEXEC) {
        return Err(Error::MissingTool {
            program: PKEXEC.to_string(),
        });
    }

    let mut args = vec![DEB_APT_TOOL.to_string()];
    args.extend(deb::apt_args(details, action, false));

    let output = exec::run_streaming(PKEXEC, &args, OPERATION_TIMEOUT, |stream, line| {
        if let Some(progress) = deb::progress_from_line(stream, line) {
            on_progress(progress);
        }
    })?;

    if output.success() {
        return Ok(());
    }

    // `pkexec` exits 126 when the authorisation dialog is dismissed or the
    // authentication fails, and 127 when the program could not be run at all.
    // Reporting either as an apt failure would be actively misleading.
    match output.code {
        Some(126) => Err(Error::NotAuthorized),
        Some(127) => Err(Error::MissingTool {
            program: DEB_APT_TOOL.to_string(),
        }),
        _ => Err(Error::CommandFailed {
            program: DEB_APT_TOOL.to_string(),
            message: apt_failure_message(&output),
        }),
    }
}

/// Pick the most informative part of a failed `apt-get` run.
///
/// apt reports unmet dependencies — by far the most common reason an install
/// fails — on *stdout*, while stderr carries lower-level noise. Taking stderr
/// alone would show the user "E: Sub-process returned an error code" and hide
/// the list of packages that actually explains it.
fn apt_failure_message(output: &exec::Output) -> String {
    let mut interesting: Vec<&str> = output
        .stdout
        .lines()
        .chain(output.stderr.lines())
        .map(str::trim)
        .filter(|line| {
            line.starts_with("E:")
                || line.starts_with("W:")
                || line.contains("unmet dependencies")
                || line.starts_with("Depends:")
                || line.starts_with("Conflicts:")
        })
        .collect();
    interesting.dedup();

    if interesting.is_empty() {
        output.failure_message()
    } else {
        interesting.join("\n")
    }
}
