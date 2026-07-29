// SPDX-License-Identifier: GPL-3.0

//! Flatpak bundle (`.flatpak`) and reference (`.flatpakref`) support.
//!
//! Flatpak differs from the other formats in ways this backend accounts for
//! rather than papers over:
//!
//! * A bundle carries its own AppStream metadata and icons, so the name,
//!   summary, licence and icon come from inside the file rather than from a
//!   control header. They are read straight out of the bundle's GVariant header
//!   by [`super::gvariant`], which is what Flatpak itself does.
//! * Dependencies are runtimes, not packages. The dependency view says "this
//!   needs `org.freedesktop.Platform//24.08`, which is / is not installed", and
//!   is honest about the fact that whether an absent runtime *could* be fetched
//!   is not a question that can be answered offline.
//! * Installation can target the user or the system, and the choice is the
//!   user's — see [`crate::config::FlatpakScope`]. A user install needs no
//!   privileges at all, so nothing here goes through [`super::privileged`]; a
//!   system install is authorised by Flatpak's own polkit actions inside its
//!   system helper, which is a better prompt than anything this application
//!   could raise because it names the operation rather than a shell command.
//! * A bundle carries no file index, and enumerating one would mean unpacking
//!   the whole thing, so [`PackageDetails::payload_known`] is false and the file
//!   list says as much instead of showing an empty list.

use std::{io::Read, path::Path, sync::RwLock};

use cosmic::widget::icon;

use super::{
    appstream, desktop, exec, gvariant, Action, Availability, Backend, Dependency,
    DependencyAlternative, DependencyKind, DependencyStatus, Error, InstalledState, OperationPlan,
    PackageDetails, PackageFormat, PlannedChange, PlannedChangeKind, Progress, Result,
};
use crate::config::FlatpakScope;
use crate::constants::{
    BUNDLE_MAX_VALUE_BYTES, FLATPAK_TOOL, INSPECT_TIMEOUT, OPERATION_TIMEOUT,
};
use crate::debug::FLATPAK;
use crate::{debug_log, fl};

/// Everything Flatpak needs is behind its own command-line tool.
const REQUIRED_TOOLS: &[&str] = &[FLATPAK_TOOL];

/// Group names in a Flatpak `metadata` file and in a `.flatpakref`.
const METADATA_APPLICATION: &str = "[Application]";
const METADATA_RUNTIME: &str = "[Runtime]";
const REF_GROUP: &str = "[Flatpak Ref]";

/// Keys of a bundle's metadata dictionary.
const KEY_REF: &str = "ref";
const KEY_METADATA: &str = "metadata";
const KEY_APPDATA: &str = "appdata";
const KEY_ICON_128: &str = "icon-128";
const KEY_ICON_64: &str = "icon-64";
const KEY_INSTALLED_SIZE: &str = "installed-size";
const KEY_RUNTIME_REPO: &str = "runtime-repo";
const KEY_ORIGIN: &str = "origin";
const KEY_COLLECTION_ID: &str = "collection-id";

/// The `extra` key under which inspection records the ref, so that every later
/// operation works from the same one rather than reassembling it differently.
const EXTRA_REF: &str = "Ref";

/// The user's configured installation scope.
///
/// Held here rather than threaded through [`Backend`] for the same reason the
/// privilege preference is: it is a property of the session, not of the package
/// being operated on.
static SCOPE: RwLock<FlatpakScope> = RwLock::new(FlatpakScope::User);

/// Record the user's scope preference. Called when the config loads and
/// whenever it changes.
pub fn set_scope(scope: FlatpakScope) {
    debug_log!(FLATPAK, "install scope set to {scope:?}");
    if let Ok(mut guard) = SCOPE.write() {
        *guard = scope;
    }
}

fn scope() -> FlatpakScope {
    SCOPE
        .read()
        .map(|guard| *guard)
        .unwrap_or(FlatpakScope::User)
}

impl FlatpakScope {
    /// The command-line flag selecting this installation.
    fn flag(self) -> &'static str {
        match self {
            Self::User => "--user",
            Self::System => "--system",
        }
    }

    /// The value Flatpak prints in its `installation` column.
    fn column_value(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::System => "system",
        }
    }
}

// ── Refs ────────────────────────────────────────────────────────────────────

/// A Flatpak ref, split into the parts the various commands want separately.
#[derive(Clone, Debug, Eq, PartialEq)]
struct Ref {
    /// `app` or `runtime`.
    kind: String,
    id: String,
    arch: String,
    branch: String,
}

impl Ref {
    /// Parse `app/org.example.Hello/x86_64/stable`.
    fn parse(text: &str) -> Option<Self> {
        let mut parts = text.split('/');
        let kind = parts.next()?.trim();
        let id = parts.next()?.trim();
        let arch = parts.next()?.trim();
        let branch = parts.next()?.trim();
        if kind.is_empty() || id.is_empty() {
            return None;
        }
        Some(Self {
            kind: kind.to_string(),
            id: id.to_string(),
            arch: arch.to_string(),
            branch: branch.to_string(),
        })
    }

    /// Parse a runtime as a `metadata` file names it, which omits the kind:
    /// `org.freedesktop.Platform/x86_64/24.08`.
    fn parse_partial(text: &str, kind: &str) -> Option<Self> {
        Self::parse(&format!("{kind}/{text}"))
    }

    fn full(&self) -> String {
        format!("{}/{}/{}/{}", self.kind, self.id, self.arch, self.branch)
    }

    /// How a runtime reads in the dependency list. The doubled slash is
    /// Flatpak's own shorthand for "on whatever architecture this is".
    fn display(&self) -> String {
        format!("{}//{}", self.id, self.branch)
    }

    fn is_runtime(&self) -> bool {
        self.kind == "runtime"
    }
}

// ── The backend ─────────────────────────────────────────────────────────────

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

    fn inspect(&self, path: &Path, _include_payload: bool) -> Result<PackageDetails> {
        // `include_payload` is ignored rather than honoured-and-left-empty:
        // neither a bundle nor a reference can produce a file list at all, and
        // the flag below is what stops that reading as "installs no files".
        let mut details = if is_reference(path) {
            inspect_reference(path)?
        } else {
            inspect_bundle(path)?
        };
        details.payload_known = false;
        Ok(details)
    }

    fn installed_state(&self, details: &PackageDetails) -> Result<InstalledState> {
        let Some(reference) = details_ref(details) else {
            return Ok(InstalledState::Unknown);
        };

        let Some(installed) = find_installed(&reference)? else {
            return Ok(InstalledState::NotInstalled);
        };
        debug_log!(
            FLATPAK,
            "{} is installed ({}) in {}",
            reference.id,
            installed.version,
            installed.installation
        );

        Ok(super::installed_state_from_versions(
            &details.version,
            &installed.version,
        ))
    }

    fn resolve_dependencies(&self, details: &mut PackageDetails) -> Result<()> {
        if details.dependencies.is_empty() {
            return Ok(());
        }

        let installed = list_installed(&["--runtime"])?;
        let remotes = configured_remotes();

        for dependency in &mut details.dependencies {
            for alternative in &mut dependency.alternatives {
                let Some(reference) = Ref::parse(&alternative.name) else {
                    continue;
                };
                alternative.status = runtime_status(&reference, &installed, &remotes);
            }
        }

        Ok(())
    }

    fn plan(&self, details: &PackageDetails, action: Action) -> Result<OperationPlan> {
        let mut plan = OperationPlan::default();

        let installed = match details_ref(details) {
            Some(reference) => find_installed(&reference)?,
            None => None,
        };
        let current_version = installed.map(|found| found.version);

        plan.changes.push(PlannedChange {
            name: details.id.clone(),
            version: (action != Action::Remove).then(|| details.version.clone()),
            current_version: current_version.clone(),
            kind: match action {
                Action::Remove => PlannedChangeKind::Remove,
                Action::Downgrade => PlannedChangeKind::Downgrade,
                Action::Upgrade => PlannedChangeKind::Upgrade,
                Action::Install | Action::Reinstall => {
                    if current_version.is_some() {
                        PlannedChangeKind::Upgrade
                    } else {
                        PlannedChangeKind::Install
                    }
                }
            },
        });

        // A runtime that is not already present is part of what the user is
        // about to put on their machine, and is by far the largest part of it
        // when it is a Platform they do not yet have.
        //
        // Whether it is present is worked out here rather than read off the
        // dependency statuses, because planning and dependency resolution are
        // started together from the same unresolved snapshot — so by the time
        // this runs the statuses are still `Unknown`, and trusting them would
        // silently leave the runtime out of every plan.
        if action.is_install() && !details.dependencies.is_empty() {
            let installed_runtimes = list_installed(&["--runtime"])?;
            for dependency in &details.dependencies {
                for alternative in &dependency.alternatives {
                    let Some(reference) = Ref::parse(&alternative.name) else {
                        continue;
                    };
                    let present = installed_runtimes
                        .iter()
                        .any(|candidate| matches_ref(&candidate.partial, &reference));
                    if !present {
                        plan.changes.push(PlannedChange {
                            name: reference.display(),
                            version: Some(reference.branch.clone()),
                            current_version: None,
                            kind: PlannedChangeKind::Install,
                        });
                    }
                }
            }
        }

        // The bundle is already on disk, so none of it is downloaded, and a
        // runtime that has to be fetched is a download whose size Flatpak will
        // not state without contacting the remote. An invented figure would be
        // worse than no line at all, so the download row is left off entirely.
        if let Some(size) = details.installed_size {
            plan.disk_size_delta = Some(match action {
                Action::Remove => -(size as i64),
                // Replacing a version with another of unknown size says nothing
                // useful about the change in disk usage.
                _ if current_version.is_some() => 0,
                _ => size as i64,
            });
        }

        debug_log!(FLATPAK, "plan: {} changes", plan.changes.len());
        Ok(plan)
    }

    fn perform(
        &self,
        details: &PackageDetails,
        action: Action,
        on_progress: &mut dyn FnMut(Progress),
    ) -> Result<()> {
        let args = if action == Action::Remove {
            uninstall_args(details)?
        } else {
            install_args(details, action)
        };
        debug_log!(FLATPAK, "{action:?} via flatpak {args:?}");

        let output = exec::run_streaming(FLATPAK_TOOL, &args, OPERATION_TIMEOUT, |_, line| {
            if let Some(progress) = progress_from_line(line) {
                on_progress(progress);
            }
        })?;

        if output.success() {
            return Ok(());
        }

        let message = output.failure_message();
        // Flatpak's system helper reports a dismissed or refused polkit prompt
        // as an ordinary error; passing that through as "flatpak failed" would
        // hide the one thing the user can actually do something about.
        if is_authorisation_failure(&message) {
            return Err(Error::NotAuthorized);
        }
        Err(Error::CommandFailed {
            program: FLATPAK_TOOL.to_string(),
            message,
        })
    }
}

/// Whether `path` is a reference rather than a bundle.
fn is_reference(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("flatpakref"))
}

/// The ref of an inspected package, read back from the field inspection stored.
fn details_ref(details: &PackageDetails) -> Option<Ref> {
    details
        .extra
        .iter()
        .find(|(key, _)| key == EXTRA_REF)
        .and_then(|(_, value)| Ref::parse(value))
}

// ── Bundle inspection ───────────────────────────────────────────────────────

fn inspect_bundle(path: &Path) -> Result<PackageDetails> {
    let header = gvariant::read_header(path)?;
    if !header.is_flatpak_bundle() {
        return Err(Error::Parse {
            detail: "the file carries no Flatpak bundle marker".to_string(),
        });
    }

    let reference = header
        .text(KEY_REF)
        .and_then(Ref::parse)
        .ok_or_else(|| Error::Parse {
            detail: "the bundle declares no usable ref".to_string(),
        })?;
    debug_log!(FLATPAK, "bundle ref {}", reference.full());

    let metadata = header.text(KEY_METADATA).unwrap_or_default().to_string();
    let component = header
        .bytes(KEY_APPDATA)
        .and_then(decompress)
        .and_then(|xml| appstream::parse(&xml, Some(&reference.id)));

    // The branch is the only version a bundle is guaranteed to have. AppStream
    // supplies the real one wherever the application bothered to declare it,
    // and that is the one the user recognises: "1.2.3", not "stable".
    let version = component
        .as_ref()
        .and_then(|component| component.version.clone())
        .unwrap_or_else(|| reference.branch.clone());

    let mut details =
        PackageDetails::new(PackageFormat::Flatpak, path, reference.id.clone(), version);

    if let Some(component) = &component {
        if let Some(name) = &component.name {
            details.name = name.clone();
        }
        details.summary = component.summary.clone();
        details.description = component.description.clone();
        details.license = component.license.clone();
        details.maintainer = component.developer.clone();
        details.homepage = component.homepage.clone();
    }

    details.architecture = Some(reference.arch.clone()).filter(|arch| !arch.is_empty());
    details.installed_size = header.number(KEY_INSTALLED_SIZE);
    details.icon = header
        .bytes(KEY_ICON_128)
        .or_else(|| header.bytes(KEY_ICON_64))
        .map(|bytes| icon::from_raster_bytes(bytes.to_vec()));

    let group = if reference.is_runtime() {
        METADATA_RUNTIME
    } else {
        METADATA_APPLICATION
    };
    let runtime = desktop::field_in(&metadata, group, "runtime");

    details.dependencies = runtime
        .as_deref()
        .and_then(|runtime| Ref::parse_partial(runtime, "runtime"))
        .map(|runtime| {
            vec![Dependency {
                kind: DependencyKind::Depends,
                alternatives: vec![DependencyAlternative {
                    name: runtime.full(),
                    constraint: None,
                    status: DependencyStatus::Unknown,
                }],
            }]
        })
        .unwrap_or_default();

    // Untranslated field names, as with the leftover control fields of a
    // `.deb`: these are Flatpak's own vocabulary and a user comparing them with
    // `flatpak info` output should see the same words.
    let mut extra = vec![
        (EXTRA_REF.to_string(), reference.full()),
        ("Branch".to_string(), reference.branch.clone()),
    ];
    for (label, value) in [
        ("Runtime", runtime),
        ("Sdk", desktop::field_in(&metadata, group, "sdk")),
        ("Command", desktop::field_in(&metadata, group, "command")),
        ("Origin", header.text(KEY_ORIGIN).map(str::to_string)),
        (
            "Runtime-Repo",
            header.text(KEY_RUNTIME_REPO).map(str::to_string),
        ),
        (
            "Collection-Id",
            header
                .text(KEY_COLLECTION_ID)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
        ),
    ] {
        if let Some(value) = value.filter(|value| !value.is_empty()) {
            extra.push((label.to_string(), value));
        }
    }
    details.extra = extra;

    Ok(details)
}

/// Inflate an AppStream blob, which Flatpak stores gzip-compressed.
///
/// Older bundles store it as plain text, so the gzip header is checked rather
/// than assumed. The output is bounded: its size comes from the file being
/// inspected, and a decompression bomb should cost a truncated description
/// rather than the machine's memory.
fn decompress(bytes: &[u8]) -> Option<String> {
    const GZIP_MAGIC: [u8; 2] = [0x1f, 0x8b];

    if !bytes.starts_with(&GZIP_MAGIC) {
        return Some(String::from_utf8_lossy(bytes).into_owned());
    }

    let mut text = String::new();
    flate2::read::GzDecoder::new(bytes)
        .take(BUNDLE_MAX_VALUE_BYTES)
        .read_to_string(&mut text)
        .ok()?;
    Some(text)
}

// ── Reference inspection ────────────────────────────────────────────────────

/// Read a `.flatpakref`, which is a small key/value file describing where to
/// get an application rather than the application itself.
///
/// Nothing here contacts the network. A `.flatpakref` names its remote and its
/// icon by URL, and fetching either during inspection would turn opening a file
/// into a network round trip that can stall — on the one path whose whole point
/// is that the window appears immediately.
fn inspect_reference(path: &Path) -> Result<PackageDetails> {
    let text = std::fs::read_to_string(path).map_err(|error| Error::Parse {
        detail: format!("cannot read {}: {error}", path.display()),
    })?;

    let field = |key: &str| desktop::field_in(&text, REF_GROUP, key);

    let id = field("Name").ok_or_else(|| Error::Parse {
        detail: "the reference has no Name field".to_string(),
    })?;
    let branch = field("Branch").unwrap_or_default();
    let is_runtime = field("IsRuntime").is_some_and(|value| value.eq_ignore_ascii_case("true"));
    let reference = Ref {
        kind: if is_runtime { "runtime" } else { "app" }.to_string(),
        id: id.clone(),
        // A reference names no architecture: Flatpak resolves the system's own
        // when it installs. The empty component is what later matching reads as
        // "any", so an application installed for this machine is still found.
        arch: String::new(),
        branch: branch.clone(),
    };
    debug_log!(FLATPAK, "reference to {id}, branch {branch:?}");

    let mut details = PackageDetails::new(
        PackageFormat::Flatpak,
        path,
        id.clone(),
        if branch.is_empty() {
            fl!("version-unknown")
        } else {
            branch.clone()
        },
    );
    details.name = field("Title").unwrap_or(id);
    details.summary = field("Comment");
    details.description = field("Description");
    details.homepage = field("Homepage");

    let mut extra = vec![(EXTRA_REF.to_string(), reference.full())];
    for (label, key) in [
        ("Branch", "Branch"),
        ("Url", "Url"),
        ("Runtime-Repo", "RuntimeRepo"),
        ("Is-Runtime", "IsRuntime"),
    ] {
        if let Some(value) = field(key) {
            extra.push((label.to_string(), value));
        }
    }
    details.extra = extra;

    Ok(details)
}

// ── System queries ──────────────────────────────────────────────────────────

/// One row of `flatpak list`.
#[derive(Clone, Debug)]
struct InstalledRef {
    /// `id/arch/branch`, as Flatpak's `ref` column reports it — note that the
    /// column omits the `app/` or `runtime/` prefix a full ref carries.
    partial: String,
    version: String,
    installation: String,
}

impl InstalledRef {
    /// The flag selecting the installation this ref is in.
    fn flag(&self) -> String {
        match self.installation.as_str() {
            "user" => "--user".to_string(),
            "system" => "--system".to_string(),
            // Flatpak supports additional named system installations, which are
            // selected by name rather than by either fixed flag.
            other => format!("--installation={other}"),
        }
    }
}

/// List installed refs across every installation.
///
/// One call rather than one per scope: with neither `--user` nor `--system`
/// given, Flatpak reports both and says which is which in the `installation`
/// column, so the whole picture costs a single process.
fn list_installed(extra_args: &[&str]) -> Result<Vec<InstalledRef>> {
    let mut args = vec!["list".to_string()];
    args.extend(extra_args.iter().map(|arg| arg.to_string()));
    args.push("--columns=ref,version,branch,installation".to_string());

    let output = exec::run(FLATPAK_TOOL, &args, INSPECT_TIMEOUT)?;
    if !output.success() {
        return Err(Error::CommandFailed {
            program: FLATPAK_TOOL.to_string(),
            message: output.failure_message(),
        });
    }

    Ok(output.stdout.lines().filter_map(parse_list_row).collect())
}

/// Parse one tab-separated `flatpak list` row.
fn parse_list_row(line: &str) -> Option<InstalledRef> {
    let mut columns = line.split('\t');
    let partial = columns.next()?.trim().to_string();
    if partial.is_empty() {
        return None;
    }
    let version = columns.next().unwrap_or("").trim().to_string();
    let branch = columns.next().unwrap_or("").trim().to_string();
    let installation = columns.next().unwrap_or("").trim().to_string();

    Some(InstalledRef {
        // Plenty of applications ship no AppStream release version, and then
        // the branch is the only thing left to compare against.
        version: if version.is_empty() { branch } else { version },
        partial,
        installation,
    })
}

/// Find `reference` among the installed refs, preferring the configured scope
/// where it is installed in more than one.
fn find_installed(reference: &Ref) -> Result<Option<InstalledRef>> {
    let kind_flag = if reference.is_runtime() {
        "--runtime"
    } else {
        "--app"
    };
    let installed = list_installed(&[kind_flag])?;
    Ok(pick_installed(&installed, reference, scope()))
}

fn pick_installed(
    installed: &[InstalledRef],
    reference: &Ref,
    preferred: FlatpakScope,
) -> Option<InstalledRef> {
    let mut matches = installed
        .iter()
        .filter(|candidate| matches_ref(&candidate.partial, reference));
    let first = matches.next()?;

    // Cloned rather than borrowed so the caller is free of the list's lifetime,
    // which is a handful of small strings either way.
    Some(
        std::iter::once(first)
            .chain(matches)
            .find(|candidate| candidate.installation == preferred.column_value())
            .unwrap_or(first)
            .clone(),
    )
}

/// Whether an `id/arch/branch` from Flatpak names the same thing as `reference`.
///
/// The architecture and branch are compared only where the reference states
/// them: a `.flatpakref` names neither, and insisting on an exact match would
/// report an installed application as absent.
fn matches_ref(partial: &str, reference: &Ref) -> bool {
    let mut parts = partial.split('/');
    let Some(id) = parts.next() else {
        return false;
    };
    let (arch, branch) = (parts.next(), parts.next());

    if id != reference.id {
        return false;
    }
    if !reference.arch.is_empty() && arch.is_some_and(|arch| arch != reference.arch) {
        return false;
    }
    if !reference.branch.is_empty() && branch.is_some_and(|branch| branch != reference.branch) {
        return false;
    }
    true
}

/// Every configured remote, as a scope flag and a name.
///
/// Both installations are consulted, not only the configured one: a runtime
/// reachable from the system's remotes is reachable regardless of where the
/// application itself is about to go.
fn configured_remotes() -> Vec<(&'static str, String)> {
    let mut remotes = Vec::new();
    for flag in ["--user", "--system"] {
        let Ok(output) = exec::run(
            FLATPAK_TOOL,
            &["remotes", flag, "--columns=name"],
            INSPECT_TIMEOUT,
        ) else {
            continue;
        };
        for line in output.stdout.lines() {
            let name = line.trim();
            if !name.is_empty() {
                remotes.push((flag, name.to_string()));
            }
        }
    }
    debug_log!(FLATPAK, "{} configured remotes", remotes.len());
    remotes
}

/// Work out the status of one runtime.
fn runtime_status(
    reference: &Ref,
    installed: &[InstalledRef],
    remotes: &[(&'static str, String)],
) -> DependencyStatus {
    if let Some(found) = installed
        .iter()
        .find(|candidate| matches_ref(&candidate.partial, reference))
    {
        return DependencyStatus::Installed {
            version: found.version.clone(),
        };
    }

    // `--cached` is what keeps this offline: it answers from the remote
    // summaries already on disk instead of contacting the remote, which is the
    // difference between a dependency list that fills in and one that stalls on
    // a slow connection.
    let full = reference.full();
    for (flag, remote) in remotes {
        let Ok(output) = exec::run(
            FLATPAK_TOOL,
            &["remote-info", flag, "--cached", remote, full.as_str()],
            INSPECT_TIMEOUT,
        ) else {
            continue;
        };
        if output.success() {
            debug_log!(FLATPAK, "{full} is available from {remote}");
            return DependencyStatus::Available {
                version: reference.branch.clone(),
            };
        }
    }

    // Not installed, and in none of the remote summaries this machine has
    // cached. That is *not* the same as unobtainable: the summaries are
    // incomplete and frequently stale, and Flatpak would very likely fetch this
    // runtime without complaint. Reporting `Missing` here would be a guess —
    // and one that disables the Install button on the strength of it.
    debug_log!(FLATPAK, "{full} not installed and in no cached remote");
    DependencyStatus::Unknown
}

// ── Operations ──────────────────────────────────────────────────────────────

/// The `flatpak install` arguments for `action`.
fn install_args(details: &PackageDetails, action: Action) -> Vec<String> {
    let mut args = vec![
        "install".to_string(),
        scope().flag().to_string(),
        "--assumeyes".to_string(),
        "--noninteractive".to_string(),
    ];

    // Anything other than a first install needs `--reinstall`. Without it
    // Flatpak declines to touch an application it already has — "already
    // installed" for a bundle at the same version, and a refusal to go
    // backwards for a downgrade — which from the outside looks like the button
    // doing nothing. `--reinstall` removes and re-adds the ref; it does not
    // touch the application's own data under `~/.var/app`.
    if action != Action::Install {
        args.push("--reinstall".to_string());
    }

    args.push(
        if is_reference(Path::new(&details.path)) {
            "--from"
        } else {
            "--bundle"
        }
        .to_string(),
    );
    args.push(details.path.clone());
    args
}

/// The `flatpak uninstall` arguments for the package described by `details`.
fn uninstall_args(details: &PackageDetails) -> Result<Vec<String>> {
    let reference = details_ref(details).ok_or_else(|| Error::Parse {
        detail: "cannot uninstall without a resolvable ref".to_string(),
    })?;

    // Uninstall has to target the installation the application is actually in,
    // not the one a new install would go to: with the preference set to "user"
    // and the application installed system-wide, the configured scope names an
    // installation where there is nothing to remove.
    let target = find_installed(&reference)?
        .map(|found| found.flag())
        .unwrap_or_else(|| scope().flag().to_string());

    Ok(vec![
        "uninstall".to_string(),
        target,
        "--assumeyes".to_string(),
        "--noninteractive".to_string(),
        reference.full(),
    ])
}

/// Turn a line of `flatpak` output into a progress report.
fn progress_from_line(line: &str) -> Option<Progress> {
    // Flatpak redraws its progress line in place, so one "line" can arrive
    // carrying carriage returns, of which only the last segment is current.
    let line = line.rsplit('\r').next().unwrap_or(line).trim();
    if line.is_empty() {
        return None;
    }
    match percentage(line) {
        Some(fraction) => Some(Progress::Fraction(fraction)),
        None => Some(Progress::Status(line.to_string())),
    }
}

/// Extract a trailing percentage, e.g. the `42%` in `Installing… 42%`.
fn percentage(line: &str) -> Option<f32> {
    let digits: String = line
        .strip_suffix('%')?
        .chars()
        .rev()
        .take_while(char::is_ascii_digit)
        .collect();
    if digits.is_empty() {
        return None;
    }
    let value: f32 = digits.chars().rev().collect::<String>().parse().ok()?;
    Some((value / 100.0).clamp(0.0, 1.0))
}

/// Whether a failure message is Flatpak reporting a refused authorisation.
fn is_authorisation_failure(message: &str) -> bool {
    let message = message.to_ascii_lowercase();
    message.contains("not authorized")
        || message.contains("not authorised")
        || message.contains("authentication")
        || message.contains("dismissed")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_refs_in_both_the_full_and_partial_spellings() {
        let full = Ref::parse("app/org.example.Hello/x86_64/stable").unwrap();
        assert_eq!(full.kind, "app");
        assert_eq!(full.id, "org.example.Hello");
        assert_eq!(full.arch, "x86_64");
        assert_eq!(full.branch, "stable");
        assert!(!full.is_runtime());

        let runtime =
            Ref::parse_partial("org.freedesktop.Platform/x86_64/24.08", "runtime").unwrap();
        assert!(runtime.is_runtime());
        assert_eq!(
            runtime.full(),
            "runtime/org.freedesktop.Platform/x86_64/24.08"
        );
        assert_eq!(runtime.display(), "org.freedesktop.Platform//24.08");

        assert!(Ref::parse("org.example.Hello").is_none());
    }

    #[test]
    fn matching_ignores_components_the_reference_does_not_state() {
        let exact = Ref::parse("app/org.example.Hello/x86_64/stable").unwrap();
        assert!(matches_ref("org.example.Hello/x86_64/stable", &exact));
        assert!(!matches_ref("org.example.Hello/aarch64/stable", &exact));
        assert!(!matches_ref("org.example.Other/x86_64/stable", &exact));

        // A `.flatpakref` names no architecture, and sometimes no branch.
        let loose = Ref {
            kind: "app".to_string(),
            id: "org.example.Hello".to_string(),
            arch: String::new(),
            branch: String::new(),
        };
        assert!(matches_ref("org.example.Hello/aarch64/beta", &loose));
        assert!(!matches_ref("org.example.Other/x86_64/stable", &loose));
    }

    #[test]
    fn parses_list_rows_and_falls_back_to_the_branch_for_a_version() {
        let with_version =
            parse_list_row("com.slack.Slack/x86_64/stable\t4.51.180\tstable\tuser").unwrap();
        assert_eq!(with_version.version, "4.51.180");
        assert_eq!(with_version.flag(), "--user");

        let without = parse_list_row("org.example.Hello/x86_64/stable\t\tstable\tsystem").unwrap();
        assert_eq!(without.version, "stable");
        assert_eq!(without.flag(), "--system");

        let named = parse_list_row("org.example.Hello/x86_64/stable\t1.0\tstable\textra").unwrap();
        assert_eq!(named.flag(), "--installation=extra");

        assert!(parse_list_row("").is_none());
    }

    #[test]
    fn the_configured_scope_wins_when_a_ref_is_installed_twice() {
        let reference = Ref::parse("app/org.example.Hello/x86_64/stable").unwrap();
        let installed = vec![
            InstalledRef {
                partial: "org.example.Hello/x86_64/stable".to_string(),
                version: "1.0".to_string(),
                installation: "system".to_string(),
            },
            InstalledRef {
                partial: "org.example.Hello/x86_64/stable".to_string(),
                version: "2.0".to_string(),
                installation: "user".to_string(),
            },
        ];

        assert_eq!(
            pick_installed(&installed, &reference, FlatpakScope::User)
                .unwrap()
                .version,
            "2.0"
        );
        assert_eq!(
            pick_installed(&installed, &reference, FlatpakScope::System)
                .unwrap()
                .version,
            "1.0"
        );
        // Installed in neither.
        let other = Ref::parse("app/org.example.Absent/x86_64/stable").unwrap();
        assert!(pick_installed(&installed, &other, FlatpakScope::User).is_none());
    }

    #[test]
    fn reads_a_percentage_out_of_a_progress_line() {
        assert_eq!(percentage("Installing… 42%"), Some(0.42));
        assert_eq!(percentage("100%"), Some(1.0));
        assert_eq!(percentage("Installing…"), None);
        assert_eq!(percentage("%"), None);

        // Only the last segment of a redrawn line is current.
        assert!(matches!(
            progress_from_line("Installing… 10%\rInstalling… 90%"),
            Some(Progress::Fraction(fraction)) if (fraction - 0.9).abs() < 0.001
        ));
        assert!(progress_from_line("   ").is_none());
    }

    #[test]
    fn a_plain_text_appdata_blob_is_not_mistaken_for_gzip() {
        assert_eq!(decompress(b"<component/>").as_deref(), Some("<component/>"));
        // Truncated gzip: no description rather than a panic.
        assert!(decompress(&[0x1f, 0x8b, 0x00, 0x00]).is_none());
    }

    /// The command line is the whole of the operation, so it is pinned down
    /// here: a wrong flag is the difference between installing for one user and
    /// prompting the whole machine for a password.
    #[test]
    fn install_arguments_match_the_file_and_the_action() {
        let bundle = PackageDetails::new(
            PackageFormat::Flatpak,
            Path::new("/tmp/hello.flatpak"),
            "org.example.Hello".to_string(),
            "1.0".to_string(),
        );

        set_scope(FlatpakScope::User);
        let args = install_args(&bundle, Action::Install);
        assert_eq!(
            args,
            vec![
                "install",
                "--user",
                "--assumeyes",
                "--noninteractive",
                "--bundle",
                "/tmp/hello.flatpak",
            ]
        );
        // Never routed through pkexec: a user install needs no privileges, and
        // a system one is Flatpak's own polkit action to raise.
        assert!(!args.iter().any(|arg| arg.contains("pkexec")));

        set_scope(FlatpakScope::System);
        assert!(install_args(&bundle, Action::Install).contains(&"--system".to_string()));

        // Anything over an existing install needs --reinstall, or Flatpak
        // declines to act and the button appears to do nothing.
        for action in [Action::Reinstall, Action::Upgrade, Action::Downgrade] {
            assert!(
                install_args(&bundle, action).contains(&"--reinstall".to_string()),
                "{action:?} should force a reinstall"
            );
        }
        assert!(!install_args(&bundle, Action::Install).contains(&"--reinstall".to_string()));

        // A reference is installed from, not as a bundle.
        let reference = PackageDetails::new(
            PackageFormat::Flatpak,
            Path::new("/tmp/app.flatpakref"),
            "org.example.Hello".to_string(),
            "stable".to_string(),
        );
        let args = install_args(&reference, Action::Install);
        assert!(args.contains(&"--from".to_string()));
        assert!(!args.contains(&"--bundle".to_string()));

        set_scope(FlatpakScope::User);
    }

    #[test]
    fn references_are_told_apart_from_bundles_by_extension() {
        assert!(is_reference(Path::new("/tmp/a.flatpakref")));
        assert!(is_reference(Path::new("/tmp/a.FlatpakRef")));
        assert!(!is_reference(Path::new("/tmp/a.flatpak")));
    }
}
