// SPDX-License-Identifier: GPL-3.0

//! Package-format backends.
//!
//! Each supported file type gets a [`Backend`] that knows how to read a package
//! off disk, work out whether it is already installed, resolve its dependencies
//! against the system, and hand a privileged operation off to be performed.
//!
//! The split between *inspection* and *operation* matters: inspection is
//! unprivileged, always safe, and is what fills the window. Operations need
//! administrator rights and go through [`privileged`], which prefers PackageKit
//! and falls back to the distribution's own tools under `pkexec`.

use std::{fmt, path::Path, sync::Arc};

use cosmic::widget::icon;

use crate::fl;

pub mod appimage;
pub mod appstream;
pub mod deb;
pub mod desktop;
pub mod exec;
pub mod flatpak;
pub mod gvariant;
pub mod packagekit;
pub mod png;
pub mod privileged;
pub mod rpm;
pub mod xpm;

pub use exec::ExecError;

// ── Formats ─────────────────────────────────────────────────────────────────

/// A package file type this application knows how to read.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PackageFormat {
    Deb,
    Rpm,
    Flatpak,
    AppImage,
}

impl PackageFormat {
    /// Every format, in the order they are listed in the UI.
    pub const ALL: &'static [PackageFormat] = &[
        PackageFormat::Deb,
        PackageFormat::Rpm,
        PackageFormat::Flatpak,
        PackageFormat::AppImage,
    ];

    /// Untranslated short name, used in logs and error text.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Deb => "deb",
            Self::Rpm => "rpm",
            Self::Flatpak => "flatpak",
            Self::AppImage => "appimage",
        }
    }

    /// Display name for the UI.
    pub fn label(self) -> String {
        match self {
            Self::Deb => fl!("format-deb"),
            Self::Rpm => fl!("format-rpm"),
            Self::Flatpak => fl!("format-flatpak"),
            Self::AppImage => fl!("format-appimage"),
        }
    }

    /// File-name patterns for the open dialog.
    ///
    /// Spelled out rather than derived from [`as_str`](Self::as_str), for two
    /// reasons. The dialog's globs are matched case-sensitively while
    /// [`from_path`](Self::from_path) is not, and an AppImage is conventionally
    /// named `.AppImage` — so a single lower-case pattern would hide the very
    /// files the filter exists to show. And several formats have more than one
    /// extension, `.flatpakref` and `.ddeb` among them.
    pub fn globs(self) -> &'static [&'static str] {
        match self {
            Self::Deb => &["*.deb", "*.ddeb", "*.udeb"],
            Self::Rpm => &["*.rpm", "*.RPM"],
            Self::Flatpak => &["*.flatpak", "*.flatpakref"],
            Self::AppImage => &["*.appimage", "*.AppImage", "*.APPIMAGE"],
        }
    }

    /// Guess the format from a file name.
    ///
    /// Extension-based on purpose: the alternative is sniffing magic bytes,
    /// which for a `.flatpak` bundle or an AppImage means reading a chunk of
    /// the file before the window can appear. Anything that guesses wrong here
    /// still fails cleanly when the backend tries to read it.
    pub fn from_path(path: &Path) -> Option<Self> {
        let extension = path.extension()?.to_str()?.to_ascii_lowercase();
        match extension.as_str() {
            // `.ddeb` is a detached-debug package and `.udeb` an installer
            // package; both are ordinary Debian archives.
            "deb" | "ddeb" | "udeb" => Some(Self::Deb),
            "rpm" => Some(Self::Rpm),
            "flatpak" | "flatpakref" => Some(Self::Flatpak),
            "appimage" => Some(Self::AppImage),
            _ => None,
        }
    }
}

impl fmt::Display for PackageFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Whether the running system can work with a given format.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Availability {
    /// Everything needed is present.
    Ready,
    /// The format is understood, but these programs are missing.
    ///
    /// Carried rather than discarded so the UI can name what to install
    /// instead of just refusing.
    Missing { tools: Vec<&'static str> },
}

impl Availability {
    /// Build an availability from a list of required programs.
    pub fn from_required(tools: &[&'static str]) -> Self {
        let missing: Vec<&'static str> = tools
            .iter()
            .copied()
            .filter(|tool| !exec::have(tool))
            .collect();
        if missing.is_empty() {
            Self::Ready
        } else {
            Self::Missing { tools: missing }
        }
    }
}

// ── Errors ──────────────────────────────────────────────────────────────────

/// Anything that can go wrong reading or operating on a package.
#[derive(Debug)]
pub enum Error {
    /// The file's extension matches no known format.
    UnknownFormat { file_name: String },
    /// The format is known but the system lacks the tools to handle it.
    Unsupported {
        format: PackageFormat,
        tools: Vec<&'static str>,
    },
    /// A required program is not installed.
    MissingTool { program: String },
    /// A tool ran but reported failure.
    CommandFailed { program: String, message: String },
    /// A tool ran past its deadline and was killed.
    Timeout { program: String },
    /// A tool's output did not look the way it was expected to.
    Parse { detail: String },
    /// PackageKit could not be reached or refused the transaction.
    PackageKit { detail: String },
    /// The user dismissed the authentication prompt.
    NotAuthorized,
    /// This backend does not implement the requested operation.
    NotImplemented { format: PackageFormat },
}

impl Error {
    /// A message suitable for showing to the user, in their language.
    ///
    /// Separate from [`fmt::Display`], which stays untranslated so that debug
    /// logs read the same regardless of who produced them.
    pub fn localized(&self) -> String {
        match self {
            Self::UnknownFormat { file_name } => {
                fl!("error-unknown-format", file = file_name.as_str())
            }
            Self::Unsupported { format, tools } => fl!(
                "error-unsupported-format",
                format = format.label(),
                tools = tools.join(", ")
            ),
            Self::MissingTool { program } => {
                fl!("error-missing-tool", program = program.as_str())
            }
            Self::CommandFailed { program, message } => fl!(
                "error-command-failed",
                program = program.as_str(),
                detail = message.as_str()
            ),
            Self::Timeout { program } => fl!("error-timeout", program = program.as_str()),
            Self::Parse { detail } => fl!("error-parse", detail = detail.as_str()),
            Self::PackageKit { detail } => fl!("error-packagekit", detail = detail.as_str()),
            Self::NotAuthorized => fl!("error-not-authorized"),
            Self::NotImplemented { format } => {
                fl!("error-not-implemented", format = format.label())
            }
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownFormat { file_name } => write!(f, "unknown package format: {file_name}"),
            Self::Unsupported { format, tools } => {
                write!(f, "{format} unsupported, missing: {}", tools.join(", "))
            }
            Self::MissingTool { program } => write!(f, "missing tool: {program}"),
            Self::CommandFailed { program, message } => {
                write!(f, "{program} failed: {message}")
            }
            Self::Timeout { program } => write!(f, "{program} timed out"),
            Self::Parse { detail } => write!(f, "parse error: {detail}"),
            Self::PackageKit { detail } => write!(f, "packagekit error: {detail}"),
            Self::NotAuthorized => f.write_str("not authorized"),
            Self::NotImplemented { format } => write!(f, "not implemented for {format}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<ExecError> for Error {
    fn from(value: ExecError) -> Self {
        match value {
            ExecError::Timeout { program, .. } => Self::Timeout { program },
            ExecError::Spawn { program, source } if source.kind() == std::io::ErrorKind::NotFound => {
                Self::MissingTool { program }
            }
            ExecError::Spawn { program, source } => Self::CommandFailed {
                program,
                message: source.to_string(),
            },
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

// ── Package model ───────────────────────────────────────────────────────────

/// One entry in a package's payload.
#[derive(Clone, Debug)]
pub struct PayloadEntry {
    /// Absolute path the entry will occupy once installed.
    pub path: String,
    /// Where a symlink points, if this entry is one.
    pub link_target: Option<String>,
    /// Whether this entry is a directory.
    pub is_directory: bool,
    /// Size in bytes, when the format reports one.
    pub size: Option<u64>,
}

/// How a dependency relates to the package that declares it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DependencyKind {
    /// Must be present, and configured before this package is configured.
    PreDepends,
    /// Must be present.
    Depends,
    /// Installed by default but not required.
    Recommends,
    /// Not installed, offered as related.
    Suggests,
    /// Cannot be installed at the same time.
    Conflicts,
    /// Cannot be *unpacked* at the same time.
    Breaks,
    /// Replaces files belonging to another package.
    Replaces,
    /// Virtual package names this package satisfies.
    Provides,
}

impl DependencyKind {
    pub fn label(self) -> String {
        match self {
            Self::PreDepends => fl!("dep-pre-depends"),
            Self::Depends => fl!("dep-depends"),
            Self::Recommends => fl!("dep-recommends"),
            Self::Suggests => fl!("dep-suggests"),
            Self::Conflicts => fl!("dep-conflicts"),
            Self::Breaks => fl!("dep-breaks"),
            Self::Replaces => fl!("dep-replaces"),
            Self::Provides => fl!("dep-provides"),
        }
    }

    /// Whether failing to satisfy this kind blocks installation.
    pub fn is_required(self) -> bool {
        matches!(self, Self::Depends | Self::PreDepends)
    }

    /// Whether a satisfied relationship of this kind is bad news.
    ///
    /// `Conflicts` and `Breaks` invert the usual reading: a package that *is*
    /// installed is the problem, and one that is nowhere to be found is the
    /// good outcome. Marking a present conflict with the same tick used for a
    /// satisfied dependency would tell the user the opposite of the truth.
    pub fn is_negative(self) -> bool {
        matches!(self, Self::Conflicts | Self::Breaks)
    }

    /// The order the dependency sections appear in the UI.
    pub const DISPLAY_ORDER: &'static [DependencyKind] = &[
        DependencyKind::PreDepends,
        DependencyKind::Depends,
        DependencyKind::Recommends,
        DependencyKind::Suggests,
        DependencyKind::Conflicts,
        DependencyKind::Breaks,
        DependencyKind::Replaces,
        DependencyKind::Provides,
    ];
}

/// Whether a single dependency is satisfied, and how it would be.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DependencyStatus {
    /// Present on the system at this version.
    Installed { version: String },
    /// Not installed, but the package manager can fetch this version.
    Available { version: String },
    /// Not installed and not a real package, but something installable
    /// provides the name.
    ProvidedBy { providers: Vec<String> },
    /// Neither installed nor obtainable.
    Missing,
    /// Not looked up — either resolution has not run yet, or the backend
    /// cannot check (no package manager for this format).
    Unknown,
}

impl DependencyStatus {
    /// Icon name representing the status in a dependency row.
    pub fn icon_name(&self) -> &'static str {
        match self {
            Self::Installed { .. } => "object-select-symbolic",
            Self::Available { .. } | Self::ProvidedBy { .. } => "list-add-symbolic",
            Self::Missing => "dialog-error-symbolic",
            Self::Unknown => "dialog-question-symbolic",
        }
    }

    /// Short description shown beside a dependency.
    pub fn label(&self) -> String {
        match self {
            Self::Installed { version } => fl!("dep-status-installed", version = version.as_str()),
            Self::Available { version } => fl!("dep-status-available", version = version.as_str()),
            Self::ProvidedBy { providers } => {
                fl!("dep-status-provided-by", providers = providers.join(", "))
            }
            Self::Missing => fl!("dep-status-missing"),
            Self::Unknown => fl!("dep-status-unknown"),
        }
    }
}

/// A single declared dependency.
///
/// Debian allows alternatives (`exim4 | mail-transport-agent`), so a dependency
/// is a *set* of candidates of which any one satisfies it. Formats without
/// alternatives simply produce a single-element list.
#[derive(Clone, Debug)]
pub struct Dependency {
    pub kind: DependencyKind,
    /// The alternatives, in the order the package listed them.
    pub alternatives: Vec<DependencyAlternative>,
}

impl Dependency {
    /// The best status among the alternatives, which is the status of the
    /// dependency as a whole.
    ///
    /// "Best" follows the order of [`DependencyStatus`]: already installed beats
    /// available, which beats provided, which beats missing. An `Unknown` only
    /// wins if nothing better exists, so a partially-resolved dependency still
    /// reports the good news it has.
    pub fn status(&self) -> DependencyStatus {
        let rank = |status: &DependencyStatus| match status {
            DependencyStatus::Installed { .. } => 0,
            DependencyStatus::Available { .. } => 1,
            DependencyStatus::ProvidedBy { .. } => 2,
            DependencyStatus::Unknown => 3,
            DependencyStatus::Missing => 4,
        };
        self.alternatives
            .iter()
            .map(|alternative| &alternative.status)
            .min_by_key(|status| rank(status))
            .cloned()
            .unwrap_or(DependencyStatus::Unknown)
    }

    /// Whether this dependency stops the package being installed.
    pub fn blocks_install(&self) -> bool {
        self.kind.is_required() && matches!(self.status(), DependencyStatus::Missing)
    }
}

/// One candidate that can satisfy a [`Dependency`].
#[derive(Clone, Debug)]
pub struct DependencyAlternative {
    pub name: String,
    /// Version constraint as declared, e.g. `>= 2.34`.
    pub constraint: Option<String>,
    pub status: DependencyStatus,
}

impl DependencyAlternative {
    /// The candidate rendered the way the package declared it.
    pub fn display(&self) -> String {
        match &self.constraint {
            Some(constraint) => format!("{} ({})", self.name, constraint),
            None => self.name.clone(),
        }
    }
}

/// Everything read out of a package file.
#[derive(Clone, Debug)]
pub struct PackageDetails {
    pub format: PackageFormat,
    /// Absolute path of the file that was opened.
    pub path: String,
    /// Identifier the package manager knows it by — a package name for
    /// deb/rpm, an application ID for flatpak, the file stem for an AppImage.
    pub id: String,
    /// Human-readable name, from a bundled desktop entry where one exists and
    /// falling back to the package name.
    pub name: String,
    pub version: String,
    pub summary: Option<String>,
    pub description: Option<String>,
    pub architecture: Option<String>,
    pub maintainer: Option<String>,
    pub homepage: Option<String>,
    pub license: Option<String>,
    pub section: Option<String>,
    /// Size the package will occupy once installed, in bytes.
    pub installed_size: Option<u64>,
    /// Size of the file on disk, in bytes.
    pub file_size: Option<u64>,
    /// Icon extracted from the package, if it carries one.
    pub icon: Option<icon::Handle>,
    pub payload: Vec<PayloadEntry>,
    /// Whether [`payload`](Self::payload) is a real answer.
    ///
    /// A `.deb` lists its contents in an index the archive carries; a Flatpak
    /// bundle does not, and enumerating one would mean unpacking the whole
    /// thing. An empty list and "this format cannot say" are very different
    /// statements to make to someone asking what a package installs, so the two
    /// are not allowed to look the same.
    pub payload_known: bool,
    pub dependencies: Vec<Dependency>,
    /// Remaining format-specific fields, shown verbatim under "Other".
    pub extra: Vec<(String, String)>,
}

impl PackageDetails {
    /// A minimally-populated record, for backends to fill in.
    pub fn new(format: PackageFormat, path: &Path, id: String, version: String) -> Self {
        let file_size = std::fs::metadata(path).ok().map(|m| m.len());
        Self {
            format,
            path: path.to_string_lossy().into_owned(),
            name: id.clone(),
            id,
            version,
            summary: None,
            description: None,
            architecture: None,
            maintainer: None,
            homepage: None,
            license: None,
            section: None,
            installed_size: None,
            file_size,
            icon: None,
            payload: Vec::new(),
            payload_known: true,
            dependencies: Vec::new(),
            extra: Vec::new(),
        }
    }

    /// Dependencies of one kind, in declaration order.
    pub fn dependencies_of(&self, kind: DependencyKind) -> impl Iterator<Item = &Dependency> {
        self.dependencies
            .iter()
            .filter(move |dependency| dependency.kind == kind)
    }

    /// Required dependencies that cannot be satisfied at all.
    pub fn unsatisfiable(&self) -> Vec<&Dependency> {
        self.dependencies
            .iter()
            .filter(|dependency| dependency.blocks_install())
            .collect()
    }
}

/// Whether the package in the file is already on the system.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InstalledState {
    /// Nothing by this name is installed.
    NotInstalled,
    /// Exactly this version is installed.
    SameVersion { installed: String },
    /// An older version is installed — the file is an upgrade.
    Older { installed: String },
    /// A newer version is installed — the file is a downgrade.
    Newer { installed: String },
    /// The backend could not tell, e.g. no package database for this format.
    Unknown,
}

impl InstalledState {
    /// The version currently on the system, if any.
    pub fn installed_version(&self) -> Option<&str> {
        match self {
            Self::SameVersion { installed }
            | Self::Older { installed }
            | Self::Newer { installed } => Some(installed),
            Self::NotInstalled | Self::Unknown => None,
        }
    }

    /// Whether anything by this name is present.
    pub fn is_installed(&self) -> bool {
        self.installed_version().is_some()
    }

    /// The action the primary button should offer.
    pub fn primary_action(&self) -> Action {
        match self {
            Self::NotInstalled | Self::Unknown => Action::Install,
            Self::SameVersion { .. } => Action::Reinstall,
            Self::Older { .. } => Action::Upgrade,
            Self::Newer { .. } => Action::Downgrade,
        }
    }
}

/// A privileged operation the user can ask for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    Install,
    Reinstall,
    Upgrade,
    Downgrade,
    Remove,
}

impl Action {
    pub fn label(self) -> String {
        match self {
            Self::Install => fl!("action-install"),
            Self::Reinstall => fl!("action-reinstall"),
            Self::Upgrade => fl!("action-upgrade"),
            Self::Downgrade => fl!("action-downgrade"),
            Self::Remove => fl!("action-remove"),
        }
    }

    /// Present-tense description shown while the operation runs.
    pub fn progress_label(self) -> String {
        match self {
            Self::Install => fl!("progress-installing"),
            Self::Reinstall => fl!("progress-reinstalling"),
            Self::Upgrade => fl!("progress-upgrading"),
            Self::Downgrade => fl!("progress-downgrading"),
            Self::Remove => fl!("progress-removing"),
        }
    }

    /// Whether this action puts the package file onto the system, as opposed to
    /// taking an installed package off it.
    pub fn is_install(self) -> bool {
        !matches!(self, Self::Remove)
    }
}

/// One package the package manager would touch to carry out an action.
#[derive(Clone, Debug)]
pub struct PlannedChange {
    pub name: String,
    /// Version that would be installed, where one applies.
    pub version: Option<String>,
    /// Version currently installed, when this is an upgrade or a removal.
    pub current_version: Option<String>,
    pub kind: PlannedChangeKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlannedChangeKind {
    Install,
    Upgrade,
    Downgrade,
    Remove,
}

impl PlannedChangeKind {
    pub fn label(self) -> String {
        match self {
            Self::Install => fl!("plan-install"),
            Self::Upgrade => fl!("plan-upgrade"),
            Self::Downgrade => fl!("plan-downgrade"),
            Self::Remove => fl!("plan-remove"),
        }
    }

    pub fn icon_name(self) -> &'static str {
        match self {
            Self::Install => "list-add-symbolic",
            Self::Upgrade => "go-up-symbolic",
            Self::Downgrade => "go-down-symbolic",
            Self::Remove => "list-remove-symbolic",
        }
    }
}

/// What the package manager says would happen if the action were performed.
///
/// This is the honest answer to "what am I actually about to install", which
/// the declared dependency list alone cannot give: it accounts for what is
/// already present, for alternatives the resolver picked, and for anything
/// pulled in transitively.
#[derive(Clone, Debug, Default)]
pub struct OperationPlan {
    pub changes: Vec<PlannedChange>,
    /// Bytes to be downloaded, when the package manager reports it.
    pub download_size: Option<u64>,
    /// Change in disk usage, in bytes. Negative when space is freed.
    pub disk_size_delta: Option<i64>,
    /// Why the operation cannot go ahead, if it cannot.
    ///
    /// Populated straight from the package manager's own explanation, which is
    /// far more use than a generic "unmet dependencies".
    pub blocked: Option<String>,
}

impl OperationPlan {
    /// Packages that would be newly installed alongside the one requested.
    pub fn additional_count(&self, requested: &str) -> usize {
        self.changes
            .iter()
            .filter(|change| change.name != requested)
            .count()
    }
}

/// Progress of a running operation, as reported by whichever transport is
/// carrying it out.
#[derive(Clone, Debug)]
pub enum Progress {
    /// Overall completion, 0.0 to 1.0.
    Fraction(f32),
    /// A line of output, or a description of the current step.
    Status(String),
}

// ── The backend trait ───────────────────────────────────────────────────────

/// Everything a package format must be able to do.
///
/// Methods are synchronous and may block for seconds; callers run them on a
/// blocking worker rather than the UI thread.
pub trait Backend: fmt::Debug + Send + Sync {
    /// Whether the system has what this backend needs.
    fn availability(&self) -> Availability;

    /// Read the package file. Does not touch the system's package database.
    fn inspect(&self, path: &Path, include_payload: bool) -> Result<PackageDetails>;

    /// Determine whether `details` is already installed.
    fn installed_state(&self, details: &PackageDetails) -> Result<InstalledState>;

    /// Fill in [`DependencyStatus`] for every dependency of `details`.
    ///
    /// Takes `&mut` because the statuses belong on the dependencies themselves;
    /// resolving is a separate step from inspection so the window can appear
    /// with the metadata visible while this runs.
    fn resolve_dependencies(&self, details: &mut PackageDetails) -> Result<()>;

    /// Ask the package manager what `action` would actually do.
    fn plan(&self, details: &PackageDetails, action: Action) -> Result<OperationPlan>;

    /// Carry out `action`, reporting progress through `on_progress`.
    ///
    /// This is the only method that requires administrator rights.
    fn perform(
        &self,
        details: &PackageDetails,
        action: Action,
        on_progress: &mut dyn FnMut(Progress),
    ) -> Result<()>;
}

/// Build the backend for `format`.
pub fn backend_for(format: PackageFormat) -> Arc<dyn Backend> {
    match format {
        PackageFormat::Deb => Arc::new(deb::DebBackend::new()),
        PackageFormat::Rpm => Arc::new(rpm::RpmBackend::new()),
        PackageFormat::Flatpak => Arc::new(flatpak::FlatpakBackend::new()),
        PackageFormat::AppImage => Arc::new(appimage::AppImageBackend::new()),
    }
}

/// Build the backend that handles `path`.
pub fn backend_for_path(path: &Path) -> Result<Arc<dyn Backend>> {
    let format = PackageFormat::from_path(path).ok_or_else(|| Error::UnknownFormat {
        file_name: path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.to_string_lossy().into_owned()),
    })?;
    crate::debug_log!(
        crate::debug::BACKEND,
        "{} is a {format} package",
        path.display()
    );

    let backend = backend_for(format);
    match backend.availability() {
        Availability::Ready => Ok(backend),
        Availability::Missing { tools } => {
            crate::debug_log!(
                crate::debug::BACKEND,
                "{format} unsupported here, missing {tools:?}"
            );
            Err(Error::Unsupported { format, tools })
        }
    }
}

/// Availability of every format, for the "supported formats" view.
pub fn all_availability() -> Vec<(PackageFormat, Availability)> {
    PackageFormat::ALL
        .iter()
        .map(|&format| (format, backend_for(format).availability()))
        .collect()
}

// ── Shared helpers ──────────────────────────────────────────────────────────

/// Format a byte count for display, using the units people expect from a
/// package manager (powers of 1000, matching apt and dnf).
pub fn format_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "kB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1000.0 && unit + 1 < UNITS.len() {
        value /= 1000.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} {}", UNITS[0])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// Build a toolkit icon from bytes pulled out of a package, given the name the
/// package filed them under.
///
/// Every backend that extracts an icon goes through here, so what counts as a
/// usable icon is decided once. Anything not listed is refused rather than
/// handed over and left to fail at draw time, where the failure is silent: an
/// icon that cannot be decoded draws nothing and says nothing about why.
///
/// Returning `None` is a normal outcome, not an error. Callers have a list of
/// candidates and should move on to the next one.
pub fn icon_from_bytes(name: &str, bytes: Vec<u8>) -> Option<icon::Handle> {
    if bytes.is_empty() {
        return None;
    }

    // The content decides, not the file name. An AppImage's `.DirIcon` is a
    // real file with no extension at all, and a mis-named icon should not get
    // to pick its decoder. Both checks are a few bytes of prefix.
    if bytes.starts_with(&png::SIGNATURE) {
        return Some(icon::from_raster_bytes(bytes));
    }
    if let Some(image) = xpm::decode(&bytes) {
        // Decoded here rather than by the toolkit, which has no idea what an
        // XPM is; see [`xpm`] for why that is worth doing at all.
        crate::debug_log!(
            crate::debug::ICON,
            "decoded a {}x{} XPM",
            image.width,
            image.height
        );
        return Some(icon::from_raster_pixels(
            image.width,
            image.height,
            image.rgba,
        ));
    }

    // SVG has no magic number, so the name gets a say after all — backed up by
    // a look for the root element, for an SVG hiding behind a bare name.
    let name = name.to_ascii_lowercase();
    if name.ends_with(".svg") || looks_like_svg(&bytes) {
        return Some(icon::from_svg_bytes(bytes));
    }

    crate::debug_log!(
        crate::debug::ICON,
        "ignoring icon {name}: not a format that can be rendered"
    );
    None
}

/// Whether `bytes` plausibly starts an SVG document.
///
/// Looks for the `<svg` root element near the start rather than for an XML
/// declaration, which every AppStream file also has and an SVG may lack.
fn looks_like_svg(bytes: &[u8]) -> bool {
    let head = &bytes[..bytes.len().min(512)];
    head.windows(4).any(|window| window == b"<svg")
}

/// Compare two version strings, ordering `left` relative to `right`.
///
/// For `.deb` this job is delegated to `dpkg`, which is the only thing that
/// gets Debian epochs and tildes right. Flatpak and AppImage have no such
/// authority to defer to: a Flatpak version is whatever the AppStream data
/// says, and an AppImage's is whatever the desktop entry or the file name says.
/// So they get this — the same digit/non-digit alternation everyone else uses,
/// and no pretence of understanding a scheme that isn't dotted numbers.
///
/// Runs of digits compare numerically, so `1.10` correctly sorts after `1.9`,
/// and are compared without parsing so a version component too long for any
/// integer type does not silently wrap.
pub fn compare_version_strings(left: &str, right: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;

    let mut left = left.trim();
    let mut right = right.trim();

    loop {
        // Separators carry no ordering of their own; only the runs between them
        // do, and treating `1.2` and `1-2` as different versions would be
        // inventing a distinction neither format makes.
        left = left.trim_start_matches(is_version_separator);
        right = right.trim_start_matches(is_version_separator);

        if left.is_empty() || right.is_empty() {
            // One version has run out while the other has more to say. What
            // that means depends on what the other says next.
            //
            // If it continues with digits, it is a further component and the
            // shorter version is the older one: `1.2` precedes `1.2.1`.
            //
            // If it continues with letters, that is a pre-release marker and
            // the *shorter* version is the newer one: `1.0` follows `1.0rc1`.
            // This is SemVer's rule and the opposite of rpm's, which treats any
            // trailing text as newer. SemVer is the right one here — these are
            // AppStream release versions and AppImage file names, where `-rc1`
            // and `-beta` mean what SemVer says they mean, and calling a
            // release candidate an upgrade over the release would offer the
            // user exactly the wrong button.
            let remainder = if left.is_empty() { right } else { left };
            let is_prerelease = remainder.starts_with(|c: char| c.is_ascii_alphabetic());
            let shorter_first = if is_prerelease {
                Ordering::Greater
            } else {
                Ordering::Less
            };
            return if left.is_empty() && right.is_empty() {
                Ordering::Equal
            } else if left.is_empty() {
                shorter_first
            } else {
                shorter_first.reverse()
            };
        }

        let (left_run, left_rest) = take_run(left);
        let (right_run, right_rest) = take_run(right);

        let left_numeric = left_run.starts_with(|c: char| c.is_ascii_digit());
        let right_numeric = right_run.starts_with(|c: char| c.is_ascii_digit());

        let ordering = match (left_numeric, right_numeric) {
            (true, true) => {
                // Leading zeros are padding, not magnitude.
                let left_digits = left_run.trim_start_matches('0');
                let right_digits = right_run.trim_start_matches('0');
                left_digits
                    .len()
                    .cmp(&right_digits.len())
                    .then_with(|| left_digits.cmp(right_digits))
            }
            // A number outranks a word, which puts `1.0` after `1.0rc` rather
            // than sorting the release candidate over the release.
            (true, false) => Ordering::Greater,
            (false, true) => Ordering::Less,
            (false, false) => left_run.cmp(right_run),
        };

        if ordering != Ordering::Equal {
            return ordering;
        }

        left = left_rest;
        right = right_rest;
    }
}

fn is_version_separator(character: char) -> bool {
    !character.is_ascii_alphanumeric()
}

/// Split off the leading run of digits, or of non-digits, whichever starts the
/// string.
fn take_run(text: &str) -> (&str, &str) {
    let numeric = text.starts_with(|c: char| c.is_ascii_digit());
    let end = text
        .find(|c: char| {
            !c.is_ascii_alphanumeric() || c.is_ascii_digit() != numeric
        })
        .unwrap_or(text.len());
    text.split_at(end)
}

/// Turn a version comparison into the state the UI reports.
pub fn installed_state_from_versions(file: &str, installed: &str) -> InstalledState {
    match compare_version_strings(file, installed) {
        std::cmp::Ordering::Equal => InstalledState::SameVersion {
            installed: installed.to_string(),
        },
        std::cmp::Ordering::Greater => InstalledState::Older {
            installed: installed.to_string(),
        },
        std::cmp::Ordering::Less => InstalledState::Newer {
            installed: installed.to_string(),
        },
    }
}

/// Format a signed byte count, for a disk-usage delta that may free space.
pub fn format_size_delta(bytes: i64) -> String {
    let magnitude = format_size(bytes.unsigned_abs());
    if bytes < 0 {
        format!("−{magnitude}")
    } else {
        format!("+{magnitude}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering;

    #[test]
    fn version_components_compare_numerically() {
        // The whole reason not to compare as strings: "10" sorts before "9"
        // lexically, which would report an upgrade as a downgrade.
        assert_eq!(compare_version_strings("1.10", "1.9"), Ordering::Greater);
        assert_eq!(compare_version_strings("1.2.3", "1.2.3"), Ordering::Equal);
        assert_eq!(compare_version_strings("2.0", "10.0"), Ordering::Less);
        // Leading zeros are padding, not magnitude.
        assert_eq!(compare_version_strings("1.007", "1.7"), Ordering::Equal);
        // A version that runs out first is the older one.
        assert_eq!(compare_version_strings("1.2", "1.2.1"), Ordering::Less);
    }

    #[test]
    fn a_release_outranks_its_own_prereleases() {
        assert_eq!(compare_version_strings("1.0", "1.0rc1"), Ordering::Greater);
        assert_eq!(compare_version_strings("1.0.0-rc1", "1.0.0"), Ordering::Less);
        assert_eq!(compare_version_strings("2.0-beta", "2.0"), Ordering::Less);
        assert_eq!(
            compare_version_strings("1.0beta", "1.0alpha"),
            Ordering::Greater
        );
        // A further numeric component still means newer, which is the case the
        // pre-release rule must not swallow.
        assert_eq!(compare_version_strings("1.2.1", "1.2"), Ordering::Greater);
    }

    #[test]
    fn separators_do_not_change_the_ordering() {
        assert_eq!(compare_version_strings("1.2-3", "1.2.3"), Ordering::Equal);
        assert_eq!(compare_version_strings(" 1.2 ", "1.2"), Ordering::Equal);
        assert_eq!(compare_version_strings("v1.2", "v1.2"), Ordering::Equal);
    }

    #[test]
    fn a_component_too_long_for_an_integer_still_compares() {
        // Compared digit by digit rather than parsed, so nothing wraps.
        let long = "1.99999999999999999999999999999999";
        assert_eq!(compare_version_strings(long, "1.9"), Ordering::Greater);
        assert_eq!(compare_version_strings(long, long), Ordering::Equal);
    }

    #[test]
    fn the_installed_state_follows_the_comparison() {
        assert_eq!(
            installed_state_from_versions("2.0", "1.0"),
            InstalledState::Older {
                installed: "1.0".to_string()
            }
        );
        assert_eq!(
            installed_state_from_versions("1.0", "2.0"),
            InstalledState::Newer {
                installed: "2.0".to_string()
            }
        );
        assert_eq!(
            installed_state_from_versions("1.0", "1.0"),
            InstalledState::SameVersion {
                installed: "1.0".to_string()
            }
        );
    }

    #[test]
    fn every_format_is_matched_by_at_least_one_of_its_own_globs() {
        // A filter that does not match the files it is named after is worse
        // than no filter, and the case-sensitivity of the two sides differs.
        for format in PackageFormat::ALL {
            for glob in format.globs() {
                let name = glob.replace('*', "example");
                assert_eq!(
                    PackageFormat::from_path(Path::new(&name)),
                    Some(*format),
                    "{glob} should be recognised as {format}"
                );
            }
        }
    }
}
