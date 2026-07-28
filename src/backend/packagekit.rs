// SPDX-License-Identifier: GPL-3.0

//! PackageKit transport for privileged operations.
//!
//! PackageKit is the preferred way to install, upgrade and remove packages
//! because it already solves the hard part: it owns the polkit integration, so
//! the user gets the desktop's own authentication dialog with a sensible
//! message, and no part of this application ever runs as root. It also speaks
//! one API for both `.deb` and `.rpm`.
//!
//! It is not, however, guaranteed to be present or working — some
//! distributions ship it disabled, and some backends implement `InstallFiles`
//! poorly. [`is_available`] is therefore checked *before* an operation starts,
//! and the caller falls back to the native tools when it answers no. The choice
//! is deliberately made up front rather than after a failure: retrying a
//! half-finished package operation through a different transport is a good way
//! to make a bad situation worse.

use std::time::Duration;

use futures_util::StreamExt;
use zbus::{proxy, zvariant::OwnedObjectPath, Connection};

use super::{Error, Progress, Result};
use crate::constants::{
    PK_ERROR_GRACE, PK_ERROR_NOT_AUTHORIZED, PK_FLAG_NONE, PK_HINTS, PK_PROBE_TIMEOUT,
};
use crate::debug::PK;
use crate::debug_log;

/// PackageKit's `PkExitEnum` value for a transaction that did what was asked.
const EXIT_SUCCESS: u32 = 1;
/// `PkExitEnum` value for a transaction the user cancelled.
const EXIT_CANCELLED: u32 = 3;

/// A percentage of 101 is PackageKit's way of saying "no idea", and must not be
/// shown as a progress bar sitting past its end.
const PERCENTAGE_UNKNOWN: u32 = 101;

/// `PkFilterEnum` is exposed over D-Bus as a bitfield in which each enum value
/// `n` occupies bit `n`. `PK_FILTER_ENUM_INSTALLED` is value 2, so the filter
/// asking only for installed packages is `1 << 2`.
const FILTER_INSTALLED: u64 = 1 << 2;

/// D-Bus error names PackageKit raises when polkit declines.
///
/// These cover the case where the daemon refuses the *method call* outright.
/// They are not the only way a refusal arrives: when the daemon accepts the
/// call and then fails to authenticate, the call returns `Ok` and the refusal
/// comes back as an `ErrorCode` signal carrying
/// [`PK_ERROR_NOT_AUTHORIZED`](crate::constants::PK_ERROR_NOT_AUTHORIZED),
/// which [`drive`] handles. Both paths have to be covered.
const NOT_AUTHORIZED_ERRORS: &[&str] = &[
    "org.freedesktop.PackageKit.Transaction.RefusedByPolicy",
    "org.freedesktop.PackageKit.Transaction.NotAuthorized",
    "org.freedesktop.DBus.Error.AccessDenied",
    "org.freedesktop.DBus.Error.InteractiveAuthorizationRequired",
];

#[proxy(
    interface = "org.freedesktop.PackageKit",
    default_service = "org.freedesktop.PackageKit",
    default_path = "/org/freedesktop/PackageKit"
)]
trait PackageKit {
    /// Allocate a transaction object to drive one operation.
    fn create_transaction(&self) -> zbus::Result<OwnedObjectPath>;

    /// Read purely to confirm the daemon is alive and answering.
    #[zbus(property)]
    fn version_major(&self) -> zbus::Result<u32>;
}

#[proxy(
    interface = "org.freedesktop.PackageKit.Transaction",
    default_service = "org.freedesktop.PackageKit"
)]
trait Transaction {
    /// Tell the daemon how this transaction is being run, before starting it.
    ///
    /// The `interactive` hint is what allows polkit to prompt; see
    /// [`PK_HINTS`](crate::constants::PK_HINTS).
    fn set_hints(&self, hints: &[&str]) -> zbus::Result<()>;

    fn install_files(&self, transaction_flags: u64, full_paths: &[&str]) -> zbus::Result<()>;

    fn remove_packages(
        &self,
        transaction_flags: u64,
        package_ids: &[&str],
        allow_deps: bool,
        autoremove: bool,
    ) -> zbus::Result<()>;

    fn resolve(&self, filter: u64, packages: &[&str]) -> zbus::Result<()>;

    /// Emitted once when the transaction stops, for any reason.
    #[zbus(signal)]
    fn finished(&self, exit: u32, runtime: u32) -> zbus::Result<()>;

    /// Emitted for a failure. `details` is a human-readable explanation from
    /// the distribution's own package tools, so it is passed straight through.
    #[zbus(signal)]
    fn error_code(&self, code: u32, details: String) -> zbus::Result<()>;

    /// Emitted for each package the transaction touches.
    #[zbus(signal)]
    fn package(&self, info: u32, package_id: String, summary: String) -> zbus::Result<()>;

    #[zbus(property)]
    fn percentage(&self) -> zbus::Result<u32>;
}

/// Run `future` on a private single-threaded runtime.
///
/// The [`Backend`](super::Backend) trait is synchronous and its methods already
/// run on a blocking worker, so there is no ambient runtime to borrow here.
/// Building a small one per operation costs a thread for the duration of an
/// install, which is nothing next to the install itself, and keeps the D-Bus
/// work from depending on how the caller happens to be scheduled.
fn block_on<F: std::future::Future>(future: F) -> Result<F::Output> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|source| Error::PackageKit {
            detail: format!("could not start a runtime: {source}"),
        })?;
    Ok(runtime.block_on(future))
}

/// Translate a zbus failure, recognising a declined authentication prompt.
fn map_zbus_error(error: zbus::Error) -> Error {
    if let zbus::Error::MethodError(name, _, _) = &error {
        let name = name.as_str();
        if NOT_AUTHORIZED_ERRORS.contains(&name) {
            debug_log!(PK, "authentication declined: {name}");
            return Error::NotAuthorized;
        }
    }
    Error::PackageKit {
        detail: error.to_string(),
    }
}

/// Whether the PackageKit daemon is present and answering.
///
/// Bounded by [`PK_PROBE_TIMEOUT`] because this runs on the way to showing a
/// window, and a daemon that needs to be activated but never comes up must not
/// hold the UI hostage — the native fallback works either way.
pub fn is_available() -> bool {
    let result = block_on(async {
        let probe = async {
            let connection = Connection::system().await?;
            let proxy = PackageKitProxy::new(&connection).await?;
            proxy.version_major().await
        };
        tokio::time::timeout(PK_PROBE_TIMEOUT, probe).await
    });

    match result {
        Ok(Ok(Ok(version))) => {
            debug_log!(PK, "daemon available, version major {version}");
            true
        }
        Ok(Ok(Err(error))) => {
            debug_log!(PK, "daemon unavailable: {error}");
            false
        }
        Ok(Err(_)) => {
            debug_log!(PK, "daemon probe timed out after {PK_PROBE_TIMEOUT:?}");
            false
        }
        Err(error) => {
            debug_log!(PK, "daemon probe could not run: {error}");
            false
        }
    }
}

/// Install (or upgrade, or downgrade) a local package file.
///
/// PackageKit makes no distinction between these: `InstallFiles` on a file
/// whose package is already present replaces it, whichever direction the
/// version moves.
pub fn install_file(
    path: &str,
    timeout: Duration,
    on_progress: &mut dyn FnMut(Progress),
) -> Result<()> {
    debug_log!(PK, "InstallFiles {path}");
    let path = path.to_string();
    block_on(async {
        let connection = Connection::system().await.map_err(map_zbus_error)?;
        let transaction = new_transaction(&connection).await?;
        drive(&transaction, timeout, on_progress, |proxy| {
            let path = path.clone();
            async move { proxy.install_files(PK_FLAG_NONE, &[path.as_str()]).await }
        })
        .await
    })?
}

/// Remove an installed package by name.
pub fn remove_package(
    name: &str,
    timeout: Duration,
    on_progress: &mut dyn FnMut(Progress),
) -> Result<()> {
    debug_log!(PK, "RemovePackages {name}");
    let name = name.to_string();
    block_on(async {
        let connection = Connection::system().await.map_err(map_zbus_error)?;

        // `RemovePackages` takes PackageKit's own composite identifier
        // (`name;version;arch;data`), not a bare name, so the installed
        // package has to be resolved first.
        let package_id = resolve_installed(&connection, &name).await?;
        debug_log!(PK, "resolved {name} to {package_id}");

        let transaction = new_transaction(&connection).await?;
        drive(&transaction, timeout, on_progress, |proxy| {
            let package_id = package_id.clone();
            async move {
                proxy
                    .remove_packages(
                        PK_FLAG_NONE,
                        &[package_id.as_str()],
                        // Refuse to take dependent packages down with it. A
                        // removal that quietly uninstalls half the desktop is
                        // exactly the surprise this application exists to
                        // prevent; PackageKit reports the conflict instead.
                        false,
                        // Leave orphaned dependencies alone for the same
                        // reason — the user asked to remove one package.
                        false,
                    )
                    .await
            }
        })
        .await
    })?
}

/// Create a transaction proxy for a fresh transaction object.
///
/// The hints are set here rather than at each call site because they describe
/// the session the transaction runs in, which is the same for all of them, and
/// because a transaction that starts without them cannot be authenticated.
async fn new_transaction(connection: &Connection) -> Result<TransactionProxy<'static>> {
    let daemon = PackageKitProxy::new(connection)
        .await
        .map_err(map_zbus_error)?;
    let path = daemon.create_transaction().await.map_err(map_zbus_error)?;
    let transaction = TransactionProxy::builder(connection)
        .path(path)
        .map_err(map_zbus_error)?
        .build()
        .await
        .map_err(map_zbus_error)?;

    // A daemon too old to know the hint rejects the call; that is not worth
    // failing the operation over, since the operation may not need authorising
    // at all. The refusal it would otherwise cause is reported when it happens.
    if let Err(error) = transaction.set_hints(PK_HINTS).await {
        debug_log!(PK, "SetHints was refused: {error}");
    }

    Ok(transaction)
}

/// Find the PackageKit identifier of the installed package called `name`.
async fn resolve_installed(connection: &Connection, name: &str) -> Result<String> {
    let transaction = new_transaction(connection).await?;

    let mut packages = transaction
        .receive_package()
        .await
        .map_err(map_zbus_error)?;
    let mut finished = transaction
        .receive_finished()
        .await
        .map_err(map_zbus_error)?;
    let mut errors = transaction
        .receive_error_code()
        .await
        .map_err(map_zbus_error)?;

    transaction
        .resolve(FILTER_INSTALLED, &[name])
        .await
        .map_err(map_zbus_error)?;

    let mut found: Option<String> = None;
    let mut failure: Option<String> = None;

    loop {
        tokio::select! {
            Some(signal) = packages.next() => {
                if let Ok(args) = signal.args() {
                    found.get_or_insert_with(|| args.package_id().to_string());
                }
            }
            Some(signal) = errors.next() => {
                if let Ok(args) = signal.args() {
                    failure = Some(args.details().to_string());
                }
            }
            Some(_) = finished.next() => break,
            else => break,
        }
    }

    found.ok_or_else(|| Error::PackageKit {
        detail: failure.unwrap_or_else(|| format!("{name} is not installed")),
    })
}

/// Start an operation on `transaction` and pump its signals until it finishes.
///
/// Signal streams are established *before* `start` is called so that a fast
/// transaction cannot finish in the gap and leave this waiting forever.
async fn drive<Start, Fut>(
    transaction: &TransactionProxy<'static>,
    timeout: Duration,
    on_progress: &mut dyn FnMut(Progress),
    start: Start,
) -> Result<()>
where
    Start: FnOnce(TransactionProxy<'static>) -> Fut,
    Fut: std::future::Future<Output = zbus::Result<()>>,
{
    let mut finished = transaction
        .receive_finished()
        .await
        .map_err(map_zbus_error)?;
    let mut errors = transaction
        .receive_error_code()
        .await
        .map_err(map_zbus_error)?;
    let mut packages = transaction
        .receive_package()
        .await
        .map_err(map_zbus_error)?;
    let mut percentages = transaction.receive_percentage_changed().await;

    start(transaction.clone()).await.map_err(map_zbus_error)?;

    let mut failure: Option<String> = None;
    let mut failure_code: Option<u32> = None;

    let outcome = tokio::time::timeout(timeout, async {
        loop {
            tokio::select! {
                Some(signal) = finished.next() => {
                    let exit = signal.args().map(|args| *args.exit()).unwrap_or(0);

                    // `Finished` routinely arrives before the `ErrorCode` that
                    // explains it, and returning here would discard the only
                    // thing that tells the user what went wrong. Wait briefly
                    // for one that is already in flight.
                    if failure.is_none() && exit != EXIT_SUCCESS {
                        if let Ok(Some(signal)) =
                            tokio::time::timeout(PK_ERROR_GRACE, errors.next()).await
                        {
                            if let Ok(args) = signal.args() {
                                debug_log!(
                                    PK,
                                    "ErrorCode {} (after Finished): {}",
                                    args.code(),
                                    args.details()
                                );
                                failure_code = Some(*args.code());
                                failure = Some(args.details().to_string());
                            }
                        }
                    }

                    return exit;
                }
                Some(signal) = errors.next() => {
                    if let Ok(args) = signal.args() {
                        let details = args.details().to_string();
                        debug_log!(PK, "ErrorCode {}: {details}", args.code());
                        on_progress(Progress::Status(details.clone()));
                        failure_code = Some(*args.code());
                        failure = Some(details);
                    }
                }
                Some(signal) = packages.next() => {
                    if let Ok(args) = signal.args() {
                        // The package id's first field is the name, which is
                        // the only part worth putting in front of a user.
                        let name = args
                            .package_id()
                            .split(';')
                            .next()
                            .unwrap_or(args.package_id())
                            .to_string();
                        on_progress(Progress::Status(name));
                    }
                }
                Some(changed) = percentages.next() => {
                    if let Ok(percentage) = changed.get().await {
                        if percentage < PERCENTAGE_UNKNOWN {
                            on_progress(Progress::Fraction(percentage as f32 / 100.0));
                        }
                    }
                }
                else => return 0,
            }
        }
    })
    .await;

    let exit = match outcome {
        Ok(exit) => exit,
        Err(_) => {
            debug_log!(PK, "transaction exceeded {timeout:?}");
            return Err(Error::Timeout {
                program: "packagekit".to_string(),
            });
        }
    };

    debug_log!(PK, "transaction finished with exit {exit}");
    match exit {
        EXIT_SUCCESS => Ok(()),
        EXIT_CANCELLED => Err(Error::NotAuthorized),
        // A refused or dismissed authentication is not a package-manager
        // failure and must not be reported as one: there is nothing wrong with
        // the package, and the message the user needs is "you were not
        // authorised", not the daemon's phrasing of it.
        _ if failure_code == Some(PK_ERROR_NOT_AUTHORIZED) => Err(Error::NotAuthorized),
        _ => Err(Error::PackageKit {
            detail: failure.unwrap_or_else(|| format!("transaction failed (exit code {exit})")),
        }),
    }
}
