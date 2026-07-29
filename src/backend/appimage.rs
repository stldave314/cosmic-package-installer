// SPDX-License-Identifier: GPL-3.0

//! AppImage (`.appimage`) support.
//!
//! AppImage is the odd one out and this backend does not pretend otherwise.
//! There is no package manager, no dependency metadata and no package database,
//! so several parts of the model degrade honestly rather than being faked:
//!
//! * "Installing" means copying the file into `~/.local/bin`, marking it
//!   executable, and putting its desktop entry and icon where the desktop can
//!   find them. None of that needs administrator rights, so nothing here goes
//!   through [`super::privileged`] and nothing ever asks for a password.
//! * "Is it installed" is answered by looking for a desktop entry this
//!   application wrote, not by querying a database. The entry records where the
//!   integrated copy went and what version it was, which is the whole of the
//!   bookkeeping.
//! * Dependencies are bundled by design, so the dependency list is empty and
//!   the view says why rather than showing an empty section.
//! * The file list is what integration will place on the system — the three
//!   files above — not the contents of the embedded filesystem, none of which
//!   is ever unpacked anywhere.
//!
//! ## Reading one means running one
//!
//! Metadata and icon live in the embedded SquashFS image, and the only tool for
//! getting them out is the AppImage's own runtime, via `--appimage-extract`.
//! That means *executing the file* — attacker-supplied code — which makes "shall
//! we read it" a security question, handled by [`Execute`]:
//!
//! * **Inspection runs it only if the user has already marked it executable.**
//!   Opening a file to look at it is not consent to run it, and a freshly
//!   downloaded AppImage that has never been `chmod +x`'d is exactly the one a
//!   cautious person is inspecting *because* they do not yet trust it. When it
//!   is not executable, the window falls back to what the file name and ELF
//!   header give and says the rest could not be read without running it.
//! * **Installation may run it,** because pressing Install is the consent that
//!   inspection lacks; a non-executable file is copied to a private temporary
//!   directory and the *copy* is marked, so the user's own file is never
//!   changed.
//!
//! Everything the runtime then writes out is read back through
//! [`Extraction::read_contained`], which refuses any path that — after symlinks
//! and `..` are resolved — leaves the extraction directory. A hostile SquashFS
//! can otherwise ship `icon.png -> /etc/shadow`, and reading it would turn
//! inspection into an arbitrary-file read.

use std::{
    fs,
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

use super::{
    appstream, desktop, exec, png, xpm, Action, Availability, Backend, Error, InstalledState,
    OperationPlan, PackageDetails, PackageFormat, PayloadEntry, PlannedChange, PlannedChangeKind,
    Progress, Result,
};
use crate::constants::{
    APPIMAGE_COPY_CHUNK, APPIMAGE_DESKTOP_DIR, APPIMAGE_EXTRACT_DIR, APPIMAGE_ICON_DIR,
    APPIMAGE_INSTALL_DIR, APPIMAGE_KEY_SOURCE, APPIMAGE_KEY_VERSION, APPIMAGE_MAGIC,
    APPIMAGE_MAGIC_OFFSET, APPIMAGE_MAX_INSPECT_COPY, APPIMAGE_MAX_METADATA_BYTES,
    DESKTOP_DATABASE_TOOL, ICON_EXTENSIONS, INSPECT_TIMEOUT,
};
use crate::debug::APPIMAGE;
use crate::{debug_log, fl};

/// AppImages are self-mounting through FUSE. Without it they can still be
/// extracted, but nothing about them will run, so its absence is worth
/// reporting up front rather than at the moment the user tries to launch one.
const FUSE_TOOLS: &[&str] = &["fusermount3", "fusermount"];

/// The icon every AppImage is supposed to carry at its root, used when the
/// desktop entry's `Icon` names nothing that is actually in the image.
const DIR_ICON: &str = ".DirIcon";

/// Where AppStream metadata sits inside a well-formed AppImage.
const METAINFO_PATTERNS: &[&str] = &["usr/share/metainfo/*.xml", "usr/share/appdata/*.xml"];

/// Permissions of an integrated AppImage: executable by its owner, readable by
/// everyone, writable by nobody but the owner.
const EXECUTABLE_MODE: u32 = 0o755;

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

    fn inspect(&self, path: &Path, include_payload: bool) -> Result<PackageDetails> {
        verify_magic(path)?;

        // Inspection does not run a file the user has not marked executable —
        // opening it to look is not consent to execute its runtime.
        let extracted = Extraction::open(path, Execute::IfUserMarkedExecutable);
        let entry = extracted.as_ref().and_then(Extraction::desktop_entry);
        let component = extracted
            .as_ref()
            .and_then(Extraction::appstream)
            .and_then(|xml| appstream::parse(&xml, None));

        let id = identifier(path, entry.as_deref(), component.as_ref());
        let version = version(path, entry.as_deref(), component.as_ref());
        debug_log!(APPIMAGE, "{} is {id} {version}", path.display());

        let mut details = PackageDetails::new(PackageFormat::AppImage, path, id.clone(), version);

        if let Some(text) = &entry {
            if let Some(name) = desktop::field(text, "Name") {
                details.name = name;
            }
            details.summary = desktop::field(text, "Comment");
        }
        if let Some(component) = &component {
            if let Some(name) = &component.name {
                details.name = name.clone();
            }
            details.summary = component.summary.clone().or(details.summary);
            details.description = component.description.clone();
            details.license = component.license.clone();
            details.maintainer = component.developer.clone();
            details.homepage = component.homepage.clone();
        }

        // An AppImage's installed size is its file size: it is copied whole.
        details.installed_size = details.file_size;
        details.icon = extracted
            .as_ref()
            .and_then(|extraction| extraction.icon(entry.as_deref()))
            .and_then(|(name, bytes)| super::icon_from_bytes(&name, bytes));

        // The dependency list is empty because an AppImage genuinely declares
        // none — everything it needs is inside it. The view says so in words
        // rather than showing an empty section.
        details.dependencies = Vec::new();

        if include_payload {
            let target = InstallTarget::for_id(&id)?;
            details.payload = target.payload(details.file_size, entry.is_some());
        }

        let mut extra = Vec::new();
        if let Some(text) = &entry {
            for key in [APPIMAGE_KEY_VERSION, "Categories"] {
                if let Some(value) = desktop::field(text, key) {
                    extra.push((key.to_string(), value));
                }
            }
        }
        if extracted.is_none() {
            // Say why the record is thin rather than letting it look as though
            // the AppImage carries nothing.
            extra.push((fl!("meta-appimage-metadata"), fl!("meta-appimage-unread")));
        }
        details.extra = extra;

        Ok(details)
    }

    fn installed_state(&self, details: &PackageDetails) -> Result<InstalledState> {
        let target = InstallTarget::for_id(&details.id)?;
        let Some(installed) = target.installed()? else {
            return Ok(InstalledState::NotInstalled);
        };

        Ok(super::installed_state_from_versions(
            &details.version,
            &installed,
        ))
    }

    fn resolve_dependencies(&self, _details: &mut PackageDetails) -> Result<()> {
        // Nothing to resolve: an AppImage bundles everything it needs, so there
        // is no list to fill in and no package manager to ask.
        Ok(())
    }

    fn plan(&self, details: &PackageDetails, action: Action) -> Result<OperationPlan> {
        let target = InstallTarget::for_id(&details.id)?;
        let current_version = target.installed()?;

        let mut plan = OperationPlan::default();
        plan.changes.push(PlannedChange {
            name: details.name.clone(),
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

        // Nothing is fetched — the file is already here — so the only figure
        // worth reporting is the change in disk usage.
        let existing = target.installed_size();
        plan.disk_size_delta = Some(match action {
            Action::Remove => -(existing.unwrap_or(0) as i64),
            _ => details.file_size.unwrap_or(0) as i64 - existing.unwrap_or(0) as i64,
        });

        Ok(plan)
    }

    fn perform(
        &self,
        details: &PackageDetails,
        action: Action,
        on_progress: &mut dyn FnMut(Progress),
    ) -> Result<()> {
        let target = InstallTarget::for_id(&details.id)?;

        if action == Action::Remove {
            target.remove(on_progress)
        } else {
            target.integrate(details, on_progress)
        }
    }
}

/// Confirm the file is an AppImage before running it.
///
/// This is the one check that has to happen first. Inspection works by
/// executing the file, and executing whatever happens to have been given an
/// `.appimage` extension is not something to do on the strength of the
/// extension alone.
fn verify_magic(path: &Path) -> Result<()> {
    let mut file = fs::File::open(path).map_err(|error| Error::Parse {
        detail: format!("cannot open {}: {error}", path.display()),
    })?;

    let mut header = [0u8; 3];
    file.seek(SeekFrom::Start(APPIMAGE_MAGIC_OFFSET))
        .and_then(|_| file.read_exact(&mut header))
        .map_err(|error| Error::Parse {
            detail: format!("cannot read the AppImage header: {error}"),
        })?;

    if header[..2] != APPIMAGE_MAGIC {
        return Err(Error::Parse {
            detail: "the file has no AppImage marker in its ELF header".to_string(),
        });
    }
    debug_log!(APPIMAGE, "AppImage type {}", header[2]);
    Ok(())
}

// ── Extraction ──────────────────────────────────────────────────────────────

/// A scratch directory that removes itself.
struct Scratch {
    path: PathBuf,
}

impl Scratch {
    fn new() -> Option<Self> {
        // Unique without a random source: the process cannot collide with
        // itself, and the timestamp separates it from any earlier run whose
        // directory outlived a crash.
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!(
            "{}-{}-{stamp}",
            env!("CARGO_PKG_NAME"),
            std::process::id()
        ));
        // `create_dir`, not `create_dir_all`: it fails if the path already
        // exists, which closes the race where a local attacker pre-creates the
        // directory — or a symlink standing in for it — to redirect the
        // extraction. On collision the inspection simply does without.
        fs::create_dir(&path).ok()?;
        Some(Self { path })
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// An AppImage opened for reading, with a scratch directory to extract into.
struct Extraction {
    scratch: Scratch,
    /// The file to run, which is the original when it is already executable and
    /// a temporary copy when it is not.
    runnable: PathBuf,
}

/// Whether reading a file's metadata is allowed to *run* it.
///
/// Reading an AppImage means executing it — `--appimage-extract` is implemented
/// by the file's own embedded runtime, which is attacker-supplied code. So the
/// decision of whether to run it is a security decision, and it is made
/// differently depending on how much the user has said they trust the file.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Execute {
    /// Only run the file if the user has already marked it executable. This is
    /// the posture for *inspection*: opening a file to look at it is not consent
    /// to run it, and a freshly downloaded AppImage that has never been
    /// `chmod +x`'d is precisely the one a cautious person is inspecting because
    /// they do not yet trust it. Its own executable bit is the only standing
    /// signal that they do.
    IfUserMarkedExecutable,
    /// Run the file, copying it and adding the executable bit to a private copy
    /// if need be. This is the posture for *integration*: pressing Install is
    /// the consent that inspection lacks. The user's own file is never modified
    /// either way — the bit is added only to a copy under the scratch directory.
    Consented,
}

impl Extraction {
    /// Prepare to read `path`, or `None` if it cannot be read — which, under
    /// [`Execute::IfUserMarkedExecutable`], includes "the user has not
    /// authorised running it".
    ///
    /// Failure here is not an error: a file that will not be run, is too large
    /// to copy, or whose runtime will not start still has a name and a size
    /// worth showing. The caller notes that the metadata could not be read and
    /// carries on.
    fn open(path: &Path, execute: Execute) -> Option<Self> {
        let scratch = Scratch::new()?;

        let runnable = if is_executable(path) {
            // The user has marked it executable, so running it is authorised
            // under either policy.
            path.to_path_buf()
        } else if execute == Execute::IfUserMarkedExecutable {
            // Not executable, and this is inspection: refuse to run it. The
            // window falls back to what the file name and ELF header give, and
            // says the rest could not be read without running the file.
            debug_log!(
                APPIMAGE,
                "{} is not executable; not running it to inspect it",
                path.display()
            );
            return None;
        } else {
            let size = fs::metadata(path).ok()?.len();
            if size > APPIMAGE_MAX_INSPECT_COPY {
                debug_log!(
                    APPIMAGE,
                    "{} is {size} bytes; too large to copy for extraction",
                    path.display()
                );
                return None;
            }
            let copy = scratch.path.join("inspect.AppImage");
            fs::copy(path, &copy).ok()?;
            set_executable(&copy).ok()?;
            debug_log!(APPIMAGE, "copied {size} bytes to run a consented AppImage");
            copy
        };

        Some(Self { scratch, runnable })
    }

    /// Run `--appimage-extract` for one pattern, returning the paths it wrote.
    ///
    /// The runtime always extracts into `squashfs-root` beside its working
    /// directory, with no way to redirect it, which is why this runs with the
    /// scratch directory as the child's working directory.
    fn extract(&self, pattern: &str) -> Vec<PathBuf> {
        let before = self.extracted_files();
        let result = exec::run_in_dir(
            self.runnable.to_string_lossy().as_ref(),
            &["--appimage-extract", pattern],
            &self.scratch.path,
            INSPECT_TIMEOUT,
        );
        if let Err(error) = result {
            debug_log!(APPIMAGE, "extracting {pattern:?} failed: {error}");
            return Vec::new();
        }

        let after = self.extracted_files();
        let new: Vec<PathBuf> = after
            .into_iter()
            .filter(|path| !before.contains(path))
            .collect();
        debug_log!(APPIMAGE, "{pattern:?} yielded {} files", new.len());
        new
    }

    /// Every file extracted so far, so that one pattern's results can be told
    /// from another's.
    fn extracted_files(&self) -> Vec<PathBuf> {
        let mut files = Vec::new();
        collect_files(&self.scratch.path.join(APPIMAGE_EXTRACT_DIR), &mut files);
        files.sort();
        files
    }

    /// The directory the runtime extracts into, and the boundary every read is
    /// held inside.
    fn root(&self) -> PathBuf {
        self.scratch.path.join(APPIMAGE_EXTRACT_DIR)
    }

    /// The bundled desktop entry, which AppImages place at their root.
    fn desktop_entry(&self) -> Option<String> {
        let candidates = self.extract("*.desktop");
        let entry = candidates
            .iter()
            .find(|path| path.extension().is_some_and(|extension| extension == "desktop"))?;
        self.read_contained(entry).and_then(|bytes| String::from_utf8(bytes).ok())
    }

    /// Read a file the extraction produced, but only if it really lies inside
    /// the extraction directory.
    ///
    /// squashfs preserves symlinks, so a hostile AppImage can ship
    /// `icon.png -> /etc/shadow` or `app.desktop -> ../../../../etc/passwd` and
    /// the runtime will lay the link down verbatim. A plain [`fs::read`] follows
    /// it, which would turn *opening* a file — no install, no prompt — into a
    /// read of anything the user can read. Resolving the real path and checking
    /// it is still within the extraction is what stops that; the legitimate
    /// `.DirIcon -> sibling.png` link resolves inside and is unaffected.
    fn read_contained(&self, path: &Path) -> Option<Vec<u8>> {
        if !path_is_within(path, &self.root()) {
            debug_log!(
                APPIMAGE,
                "refusing to read {}: it resolves outside the extraction",
                path.display()
            );
            return None;
        }

        // The file's size is attacker-chosen — the runtime wrote it out of a
        // SquashFS the AppImage controls — so it is read under a cap rather than
        // whole, or a bundle with a gigabyte-sized `metainfo.xml` would exhaust
        // memory on inspection.
        let file = fs::File::open(path).ok()?;
        let mut bytes = Vec::new();
        std::io::Read::take(file, APPIMAGE_MAX_METADATA_BYTES)
            .read_to_end(&mut bytes)
            .ok()?;
        Some(bytes)
    }

    /// The application icon, as a file name and its bytes.
    ///
    /// Tries the name the desktop entry declares before falling back to
    /// `.DirIcon`, which the specification requires every AppImage to carry.
    /// That order matters: `.DirIcon` is usually a symlink to whichever of
    /// several icons the author considered canonical, and going by the declared
    /// name reaches the same file directly.
    fn icon(&self, entry: Option<&str>) -> Option<(String, Vec<u8>)> {
        let declared = entry.and_then(|text| desktop::field(text, "Icon"));

        let mut patterns: Vec<String> = Vec::new();
        if let Some(name) = declared.as_deref().filter(|name| !name.starts_with('/')) {
            for extension in ICON_EXTENSIONS {
                patterns.push(format!("{name}.{extension}"));
                patterns.push(format!("usr/share/icons/hicolor/*/apps/{name}.{extension}"));
            }
        }
        patterns.push(DIR_ICON.to_string());

        for pattern in patterns {
            for path in self.extract(&pattern) {
                if let Some(icon) = self.read_icon(&path) {
                    return Some(icon);
                }
            }
        }
        None
    }

    /// Read one extracted icon candidate, resolving it if it is a link.
    ///
    /// Extracting `.DirIcon` on its own produces a *dangling* symlink: the
    /// runtime writes the link but not the file it points at, because that file
    /// did not match the pattern. So the link is read, its target extracted in
    /// turn, and the bytes taken from there.
    fn read_icon(&self, path: &Path) -> Option<(String, Vec<u8>)> {
        let is_link = fs::symlink_metadata(path).is_ok_and(|data| data.is_symlink());

        let resolved = if is_link {
            let target = fs::read_link(path).ok()?;
            // A symlink target is entirely attacker-controlled. It is joined
            // relative to the extraction and only extracted further when it
            // names something *inside* it — an absolute `/etc/...` target, or
            // one with enough `..` to climb out, is dropped here rather than
            // chased. `read_contained` re-checks the resolved path regardless,
            // so this is the belt to that braces.
            let target = target.to_string_lossy().into_owned();
            self.extract(&target);
            self.root().join(&target)
        } else {
            path.to_path_buf()
        };

        let bytes = self.read_contained(&resolved)?;
        if bytes.is_empty() {
            return None;
        }
        let name = resolved.file_name()?.to_string_lossy().into_owned();
        Some((name, bytes))
    }

    /// The bundled AppStream document, where the AppImage carries one.
    fn appstream(&self) -> Option<String> {
        for pattern in METAINFO_PATTERNS {
            for path in self.extract(pattern) {
                let Some(bytes) = self.read_contained(&path) else {
                    continue;
                };
                if let Ok(text) = String::from_utf8(bytes) {
                    if !text.trim().is_empty() {
                        return Some(text);
                    }
                }
            }
        }
        None
    }
}

/// Whether `path`, once every symlink and `..` in it is resolved, still lies
/// inside `base`.
///
/// This is the one check standing between "inspect an AppImage" and "read any
/// file the user can". It canonicalises both sides — which follows symlinks and
/// collapses `..` — and then asks the resolved path to start with the resolved
/// base. A path that cannot be canonicalised (a dangling link, a missing file)
/// is treated as outside, because a file that is not there is not one to read.
pub(crate) fn path_is_within(path: &Path, base: &Path) -> bool {
    let (Ok(real_path), Ok(real_base)) = (path.canonicalize(), base.canonicalize()) else {
        return false;
    };
    real_path.starts_with(&real_base)
}

/// Collect every regular file under `directory`, following nothing.
fn collect_files(directory: &Path, into: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        // `symlink_metadata` so a symlink is recorded as the file it is rather
        // than followed into a directory that may be outside the scratch area.
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            continue;
        };
        if metadata.is_dir() {
            collect_files(&path, into);
        } else {
            into.push(path);
        }
    }
}

// ── Identity ────────────────────────────────────────────────────────────────

/// The identifier the integrated copy is filed under.
///
/// AppStream first, because it is the only one of the three that is designed to
/// be unique; then the desktop entry's own name; then the file name, which at
/// least distinguishes one AppImage from another in the same directory.
fn identifier(
    path: &Path,
    entry: Option<&str>,
    component: Option<&appstream::Component>,
) -> String {
    let candidate = component
        .and_then(|component| component.id.clone())
        .or_else(|| entry.and_then(|text| desktop::field(text, "StartupWMClass")))
        .or_else(|| entry.and_then(|text| desktop::field(text, "Name")))
        .or_else(|| {
            path.file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| "appimage".to_string());

    sanitise(&candidate)
}

/// Reduce a name to something safe to use as a file name.
fn sanitise(text: &str) -> String {
    let cleaned: String = text
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_') {
                character
            } else {
                '-'
            }
        })
        .collect();
    let trimmed = cleaned.trim_matches(['-', '.']).to_string();
    if trimmed.is_empty() {
        "appimage".to_string()
    } else {
        trimmed
    }
}

/// The version to show, from whichever source has one.
///
/// AppStream wins where it exists, being the only source that is actually
/// specified to hold a version. After that the order is not simply "the
/// declared one first": `X-AppImage-Version` is routinely a git hash —
/// qFlipper 1.3.3 declares `bfce851` — which tells the user nothing and cannot
/// be compared against anything to decide whether an installed copy is older.
/// So a declared version that does not look like a version yields to one found
/// in the file name, and is used only if the name has none either.
fn version(path: &Path, entry: Option<&str>, component: Option<&appstream::Component>) -> String {
    let declared = entry.and_then(|text| desktop::field(text, APPIMAGE_KEY_VERSION));
    let in_name = path
        .file_stem()
        .and_then(|stem| version_in_name(&stem.to_string_lossy()));

    let declared_looks_like_a_version = declared.as_deref().is_some_and(looks_like_a_version);

    component
        .and_then(|component| component.version.clone())
        .or_else(|| declared.clone().filter(|_| declared_looks_like_a_version))
        .or(in_name)
        .or(declared)
        .unwrap_or_else(|| fl!("version-unknown"))
}

/// Whether a string reads as a dotted version rather than an opaque build
/// identifier.
fn looks_like_a_version(text: &str) -> bool {
    let trimmed = text.trim_start_matches(['v', 'V']);
    trimmed.starts_with(|character: char| character.is_ascii_digit())
        && trimmed.contains('.')
        && trimmed
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '+' | '_' | '~'))
}

/// Pick a dotted version out of a file name, e.g. the `1.26.24` in
/// `VeraCrypt-1.26.24-x86_64`.
///
/// Requires at least one dot between digits, which is what keeps it from
/// reporting the `64` of `x86_64` as a version.
fn version_in_name(name: &str) -> Option<String> {
    name.split(|character: char| !character.is_ascii_digit() && character != '.')
        .find(|token| {
            token.contains('.')
                && token.starts_with(|character: char| character.is_ascii_digit())
                && token.ends_with(|character: char| character.is_ascii_digit())
        })
        .map(str::to_string)
}

// ── Integration ─────────────────────────────────────────────────────────────

/// Where an AppImage goes, and what is already there.
struct InstallTarget {
    id: String,
    binary: PathBuf,
    entry: PathBuf,
    icon_root: PathBuf,
}

impl InstallTarget {
    fn for_id(id: &str) -> Result<Self> {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or_else(|| Error::Parse {
                detail: "HOME is not set, so there is nowhere to install to".to_string(),
            })?;
        Ok(Self::under(&home, id))
    }

    /// The paths an integration under `home` would occupy.
    ///
    /// Split out from [`for_id`](Self::for_id) so the whole install and removal
    /// cycle can be exercised against a scratch directory instead of the
    /// developer's own home.
    fn under(home: &Path, id: &str) -> Self {
        Self {
            id: id.to_string(),
            binary: home
                .join(APPIMAGE_INSTALL_DIR)
                .join(format!("{id}.AppImage")),
            entry: home.join(APPIMAGE_DESKTOP_DIR).join(format!("{id}.desktop")),
            icon_root: home.join(APPIMAGE_ICON_DIR),
        }
    }

    /// The version of the integrated copy, if there is one.
    ///
    /// Both halves have to be present: a desktop entry pointing at a file that
    /// is no longer there describes an install that has been half undone, and
    /// reporting it as installed would offer an Uninstall that cannot work and
    /// hide the Install that would fix it.
    fn installed(&self) -> Result<Option<String>> {
        let Ok(text) = fs::read_to_string(&self.entry) else {
            return Ok(None);
        };
        let Some(source) = desktop::field(&text, APPIMAGE_KEY_SOURCE) else {
            // A desktop entry by this name that is not one of ours. Leaving it
            // alone is the only safe thing to do with it.
            debug_log!(APPIMAGE, "{} was not written by us", self.entry.display());
            return Ok(None);
        };
        if !Path::new(&source).exists() {
            debug_log!(APPIMAGE, "integrated copy {source} has gone missing");
            return Ok(None);
        }

        Ok(Some(
            desktop::field(&text, APPIMAGE_KEY_VERSION).unwrap_or_else(|| fl!("version-unknown")),
        ))
    }

    fn installed_size(&self) -> Option<u64> {
        fs::metadata(&self.binary).ok().map(|data| data.len())
    }

    /// What integration will put on the system.
    fn payload(&self, size: Option<u64>, has_entry: bool) -> Vec<PayloadEntry> {
        let mut entries = vec![PayloadEntry {
            path: self.binary.to_string_lossy().into_owned(),
            link_target: None,
            is_directory: false,
            size,
        }];
        if has_entry {
            entries.push(PayloadEntry {
                path: self.entry.to_string_lossy().into_owned(),
                link_target: None,
                is_directory: false,
                size: None,
            });
        }
        entries
    }

    /// Copy the AppImage into place and register it with the desktop.
    fn integrate(
        &self,
        details: &PackageDetails,
        on_progress: &mut dyn FnMut(Progress),
    ) -> Result<()> {
        let source = PathBuf::from(&details.path);

        // Re-read the desktop entry and icon rather than carrying them through
        // inspection: the file may have changed on disk since, and an install
        // that writes stale metadata is worse than one that costs a second
        // extraction. Pressing Install is the consent that lets this run a file
        // inspection would have declined to.
        let extracted = Extraction::open(&source, Execute::Consented);
        let entry = extracted.as_ref().and_then(Extraction::desktop_entry);
        let icon = extracted
            .as_ref()
            .and_then(|extraction| extraction.icon(entry.as_deref()));

        create_parent(&self.binary)?;
        on_progress(Progress::Status(fl!("progress-copying")));
        copy_with_progress(&source, &self.binary, on_progress)?;
        set_executable(&self.binary)?;

        let icon_name = match icon {
            Some((name, bytes)) => self.write_icon(&name, &bytes)?,
            None => None,
        };

        on_progress(Progress::Status(fl!("progress-integrating")));
        self.write_entry(entry.as_deref(), details, icon_name)?;

        refresh_desktop_database(&self.entry);
        debug_log!(APPIMAGE, "integrated {} at {}", self.id, self.binary.display());
        Ok(())
    }

    /// Write the icon into the user's icon theme, returning the name a desktop
    /// entry should use to refer to it — or `None` when there is nothing worth
    /// writing, in which case the entry keeps whatever `Icon` it came with.
    ///
    /// What goes in is decided by content, not by the file's name: SVG and PNG
    /// are written verbatim, and an XPM is converted to PNG first. The icon
    /// theme directories are read by every program on the system, and while
    /// the theme specification still admits XPM, COSMIC itself cannot decode
    /// one — so writing the XPM out unchanged would integrate an application
    /// whose launcher entry has a hole where its icon should be.
    fn write_icon(&self, source_name: &str, bytes: &[u8]) -> Result<Option<String>> {
        // Storage for a converted icon, alive as long as `payload` borrows it.
        let converted: Vec<u8>;

        let is_svg = source_name.to_ascii_lowercase().ends_with(".svg");
        let (extension, payload): (&str, &[u8]) = if is_svg {
            ("svg", bytes)
        } else if bytes.starts_with(&png::SIGNATURE) {
            ("png", bytes)
        } else if let Some(image) = xpm::decode(bytes) {
            match png::encode_rgba(image.width, image.height, &image.rgba) {
                Some(encoded) => {
                    debug_log!(
                        APPIMAGE,
                        "converted a {}x{} XPM icon to PNG for the icon theme",
                        image.width,
                        image.height
                    );
                    converted = encoded;
                    ("png", converted.as_slice())
                }
                None => return Ok(None),
            }
        } else {
            // Nothing any icon consumer will decode; writing it would just
            // strand a dead file in the theme.
            debug_log!(APPIMAGE, "not integrating undecodable icon {source_name}");
            return Ok(None);
        };

        // The icon theme takes the directory as a statement of the icon's size,
        // so a PNG has to go in the directory matching its actual pixels or the
        // desktop will pick it for the wrong slot. An SVG has no size to get
        // wrong.
        let directory = if is_svg {
            "scalable".to_string()
        } else {
            let size = png::width(payload).unwrap_or(256);
            format!("{size}x{size}")
        };

        let path = self
            .icon_root
            .join(directory)
            .join("apps")
            .join(format!("{}.{extension}", self.id));
        create_parent(&path)?;
        write_file(&path, payload)?;
        debug_log!(APPIMAGE, "wrote icon {}", path.display());

        Ok(Some(self.id.clone()))
    }

    /// Write the desktop entry, rewritten to point at the integrated copy.
    ///
    /// An entry is always written, even for an AppImage that bundles none. Two
    /// reasons: without one there is no launcher, so the application would be
    /// installed and invisible; and the entry is where the record of *what* was
    /// installed lives, so skipping it would make the integration impossible to
    /// detect afterwards and impossible to undo from here.
    fn write_entry(
        &self,
        original: Option<&str>,
        details: &PackageDetails,
        icon_name: Option<String>,
    ) -> Result<()> {
        let binary = self.binary.to_string_lossy().into_owned();
        let synthesised;
        let original = match original {
            Some(text) => text,
            None => {
                debug_log!(
                    APPIMAGE,
                    "{} bundles no desktop entry; writing a minimal one",
                    self.id
                );
                synthesised = format!(
                    "[Desktop Entry]\nType=Application\nName={}\nTerminal=false\n",
                    details.name
                );
                &synthesised
            }
        };

        let mut overrides = vec![
            // The bundled `Exec` names a command inside the AppImage's own
            // mount, which does not exist from out here.
            ("Exec", binary.clone()),
            ("TryExec", binary.clone()),
            (APPIMAGE_KEY_SOURCE, binary),
            (APPIMAGE_KEY_VERSION, details.version.clone()),
        ];
        if let Some(name) = icon_name {
            overrides.push(("Icon", name));
        }

        let text = desktop::rewrite(original, &overrides);
        create_parent(&self.entry)?;
        write_file(&self.entry, text.as_bytes())
    }

    /// Undo an integration.
    ///
    /// Every path is removed independently and a missing one is not an error:
    /// the job is to leave nothing behind, and refusing to remove the binary
    /// because the icon had already been deleted would do the opposite.
    fn remove(&self, on_progress: &mut dyn FnMut(Progress)) -> Result<()> {
        on_progress(Progress::Status(fl!("progress-removing")));

        let mut removed = 0usize;
        for path in std::iter::once(self.binary.clone())
            .chain(std::iter::once(self.entry.clone()))
            .chain(self.icon_paths())
        {
            match fs::remove_file(&path) {
                Ok(()) => {
                    removed += 1;
                    debug_log!(APPIMAGE, "removed {}", path.display());
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(Error::CommandFailed {
                        program: "remove".to_string(),
                        message: format!("{}: {error}", path.display()),
                    })
                }
            }
        }

        refresh_desktop_database(&self.entry);
        debug_log!(APPIMAGE, "removed {removed} files for {}", self.id);
        Ok(())
    }

    /// Every icon this integration might have written, across the size
    /// directories and extensions it could have chosen.
    fn icon_paths(&self) -> Vec<PathBuf> {
        let Ok(sizes) = fs::read_dir(&self.icon_root) else {
            return Vec::new();
        };
        let mut paths = Vec::new();
        for size in sizes.flatten() {
            for extension in ICON_EXTENSIONS {
                paths.push(
                    size.path()
                        .join("apps")
                        .join(format!("{}.{extension}", self.id)),
                );
            }
        }
        paths
    }
}

/// Copy `source` to `destination`, reporting how far along it is.
///
/// Written by hand rather than with [`fs::copy`] because an AppImage is large
/// enough for the copy to be the whole of the operation's duration, and a
/// progress bar that sits at zero and then jumps to done is no better than none.
fn copy_with_progress(
    source: &Path,
    destination: &Path,
    on_progress: &mut dyn FnMut(Progress),
) -> Result<()> {
    let failed = |error: std::io::Error, path: &Path| Error::CommandFailed {
        program: "copy".to_string(),
        message: format!("{}: {error}", path.display()),
    };

    let mut input = fs::File::open(source).map_err(|error| failed(error, source))?;
    let total = input.metadata().map(|data| data.len()).unwrap_or(0);

    // Written under a temporary name and renamed into place, so an interrupted
    // copy cannot leave a truncated executable where a working one used to be.
    let staging = destination.with_extension("AppImage.part");
    let mut output = fs::File::create(&staging).map_err(|error| failed(error, &staging))?;

    let mut buffer = vec![0u8; APPIMAGE_COPY_CHUNK];
    let mut copied = 0u64;
    loop {
        let read = input.read(&mut buffer).map_err(|error| failed(error, source))?;
        if read == 0 {
            break;
        }
        output
            .write_all(&buffer[..read])
            .map_err(|error| failed(error, &staging))?;
        copied += read as u64;
        if total > 0 {
            on_progress(Progress::Fraction(copied as f32 / total as f32));
        }
    }
    output.flush().map_err(|error| failed(error, &staging))?;
    drop(output);

    fs::rename(&staging, destination).map_err(|error| {
        let _ = fs::remove_file(&staging);
        failed(error, destination)
    })
}

fn create_parent(path: &Path) -> Result<()> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    fs::create_dir_all(parent).map_err(|error| Error::CommandFailed {
        program: "mkdir".to_string(),
        message: format!("{}: {error}", parent.display()),
    })
}

fn write_file(path: &Path, bytes: &[u8]) -> Result<()> {
    fs::write(path, bytes).map_err(|error| Error::CommandFailed {
        program: "write".to_string(),
        message: format!("{}: {error}", path.display()),
    })
}

fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path)
        .map(|data| data.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

fn set_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(EXECUTABLE_MODE)).map_err(|error| {
        Error::CommandFailed {
            program: "chmod".to_string(),
            message: format!("{}: {error}", path.display()),
        }
    })
}

/// Tell the desktop about a new or removed entry.
///
/// Best-effort on purpose: without it the entry appears at the next login
/// instead of immediately, which is a worse outcome than an install that
/// reports failure over a cache that could not be refreshed.
fn refresh_desktop_database(entry: &Path) {
    let Some(directory) = entry.parent() else {
        return;
    };
    if !exec::have(DESKTOP_DATABASE_TOOL) {
        return;
    }
    let _ = exec::run(
        DESKTOP_DATABASE_TOOL,
        &["-q".as_ref(), directory.as_os_str()],
        INSPECT_TIMEOUT,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn identifiers_are_safe_to_use_as_file_names() {
        assert_eq!(sanitise("org.example.App"), "org.example.App");
        assert_eq!(sanitise("My App!"), "My-App");
        assert_eq!(sanitise("../../etc/passwd"), "etc-passwd");
        assert_eq!(sanitise("///"), "appimage");
        assert_eq!(sanitise(""), "appimage");
    }

    #[test]
    fn a_version_is_recognised_in_a_file_name() {
        assert_eq!(
            version_in_name("VeraCrypt-1.26.24-x86_64").as_deref(),
            Some("1.26.24")
        );
        assert_eq!(
            version_in_name("qFlipper-x86_64-1.3.3").as_deref(),
            Some("1.3.3")
        );
        // `x86_64` is not a version, and neither is a bare number.
        assert_eq!(version_in_name("SomeApp-x86_64"), None);
        assert_eq!(version_in_name("SomeApp"), None);
    }

    #[test]
    fn a_build_hash_is_not_mistaken_for_a_version() {
        assert!(looks_like_a_version("1.3.3"));
        assert!(looks_like_a_version("v2.0.1-rc1"));
        // qFlipper 1.3.3 declares this as its X-AppImage-Version.
        assert!(!looks_like_a_version("bfce851"));
        assert!(!looks_like_a_version("continuous"));
        assert!(!looks_like_a_version(""));
    }

    #[test]
    fn the_file_name_beats_a_declared_build_hash() {
        let path = Path::new("/tmp/qFlipper-x86_64-1.3.3.AppImage");
        let hash_entry = "[Desktop Entry]\nX-AppImage-Version=bfce851\n";
        assert_eq!(version(path, Some(hash_entry), None), "1.3.3");

        // A declared version that really is one still wins over the name.
        let real_entry = "[Desktop Entry]\nX-AppImage-Version=1.4.0\n";
        assert_eq!(version(path, Some(real_entry), None), "1.4.0");

        // With nothing else to go on, even a hash beats saying nothing.
        let bare = Path::new("/tmp/qFlipper.AppImage");
        assert_eq!(version(bare, Some(hash_entry), None), "bfce851");

        // AppStream outranks both.
        let component = appstream::Component {
            version: Some("9.9.9".to_string()),
            ..appstream::Component::default()
        };
        assert_eq!(version(path, Some(real_entry), Some(&component)), "9.9.9");
    }

    #[test]
    fn the_identifier_prefers_appstream_then_the_entry_then_the_name() {
        let entry = "[Desktop Entry]\nName=Hello World\nStartupWMClass=hello\n";
        let component = appstream::Component {
            id: Some("org.example.Hello".to_string()),
            ..appstream::Component::default()
        };

        let path = Path::new("/tmp/Hello-1.0-x86_64.AppImage");
        assert_eq!(
            identifier(path, Some(entry), Some(&component)),
            "org.example.Hello"
        );
        assert_eq!(identifier(path, Some(entry), None), "hello");
        assert_eq!(identifier(path, None, None), "Hello-1.0-x86_64");
    }

    /// Inspection must not run a file the user has not marked executable.
    ///
    /// This is the guarantee that opening an untrusted AppImage to look at it
    /// does not execute its runtime. It is checked at the door — `open` returns
    /// `None` — rather than relying on the extract later failing.
    #[test]
    fn inspection_will_not_run_a_non_executable_file() {
        let scratch = Scratch::new().unwrap();
        let file = scratch.path.join("app.AppImage");
        fs::write(&file, b"not really an appimage").unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(0o644)).unwrap();

        // Inspection declines outright.
        assert!(Extraction::open(&file, Execute::IfUserMarkedExecutable).is_none());
        // Consent (an Install) is willing to copy and run it.
        assert!(Extraction::open(&file, Execute::Consented).is_some());

        // Once the user has marked it executable, inspection will run it —
        // their bit is the standing signal of trust.
        fs::set_permissions(&file, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(Extraction::open(&file, Execute::IfUserMarkedExecutable).is_some());
    }

    /// The containment check that stands between inspecting an AppImage and
    /// reading arbitrary files through a symlink the image controls.
    #[test]
    fn extraction_reads_are_held_inside_the_extraction() {
        let scratch = Scratch::new().unwrap();
        let root = scratch.path.join(APPIMAGE_EXTRACT_DIR);
        fs::create_dir_all(&root).unwrap();

        // A file only the user can read, standing in for anything outside the
        // image the app has no business reading.
        let secret = scratch.path.join("secret");
        fs::write(&secret, b"secret").unwrap();

        // The two shapes a hostile squashfs can ship: an absolute target and a
        // relative one with enough `..` to climb out of the extraction.
        std::os::unix::fs::symlink(&secret, root.join("abs.png")).unwrap();
        std::os::unix::fs::symlink("../secret", root.join("rel.png")).unwrap();
        assert!(!path_is_within(&root.join("abs.png"), &root));
        assert!(!path_is_within(&root.join("rel.png"), &root));

        // The legitimate case — `.DirIcon -> sibling` — must still be allowed,
        // or every real AppImage that uses one loses its icon.
        fs::write(root.join("icon.png"), b"\x89PNG").unwrap();
        std::os::unix::fs::symlink("icon.png", root.join(".DirIcon")).unwrap();
        assert!(path_is_within(&root.join(".DirIcon"), &root));
        assert!(path_is_within(&root.join("icon.png"), &root));

        // A dangling link resolves to nothing, which is treated as outside.
        std::os::unix::fs::symlink("nowhere", root.join("dangling.png")).unwrap();
        assert!(!path_is_within(&root.join("dangling.png"), &root));
    }

    /// A complete integrate-then-remove cycle, against a scratch directory
    /// standing in for `$HOME`.
    ///
    /// The AppImage here is a plain file rather than a real one — the runtime
    /// cannot be made to run inside a test — so the desktop entry taken is the
    /// synthesised one. That is the path worth pinning down anyway: it is what
    /// makes an AppImage without its own entry still detectable as installed.
    #[test]
    fn integrates_and_removes_under_a_scratch_home() {
        let scratch = Scratch::new().unwrap();
        let home = scratch.path.join("home");
        let source = scratch.path.join("Example-2.0-x86_64.AppImage");
        fs::write(&source, vec![0xABu8; 4096]).unwrap();

        let mut details = PackageDetails::new(
            PackageFormat::AppImage,
            &source,
            "org.example.Example".to_string(),
            "2.0".to_string(),
        );
        details.name = "Example".to_string();

        let target = InstallTarget::under(&home, &details.id);
        assert_eq!(target.installed().unwrap(), None);

        let mut seen: Vec<Progress> = Vec::new();
        target
            .integrate(&details, &mut |progress| seen.push(progress))
            .unwrap();

        // The binary is in place, executable, and byte-for-byte the original.
        assert!(is_executable(&target.binary));
        assert_eq!(fs::read(&target.binary).unwrap(), vec![0xABu8; 4096]);
        // Nothing left over from the staged copy.
        assert!(!target.binary.with_extension("AppImage.part").exists());
        assert!(seen
            .iter()
            .any(|progress| matches!(progress, Progress::Fraction(_))));

        // The entry points at the integrated copy, not at anything inside the
        // image, and records enough to recognise the install later.
        let entry = fs::read_to_string(&target.entry).unwrap();
        let binary = target.binary.to_string_lossy().into_owned();
        assert_eq!(desktop::field(&entry, "Exec").as_deref(), Some(&*binary));
        assert_eq!(desktop::field(&entry, "Name").as_deref(), Some("Example"));
        assert_eq!(
            desktop::field(&entry, APPIMAGE_KEY_SOURCE).as_deref(),
            Some(&*binary)
        );
        assert_eq!(target.installed().unwrap().as_deref(), Some("2.0"));

        // A binary that has gone missing is not an install, however tidy the
        // desktop entry looks.
        fs::remove_file(&target.binary).unwrap();
        assert_eq!(target.installed().unwrap(), None);
        target
            .integrate(&details, &mut |_| {})
            .expect("re-integration over a half-removed install");

        target.remove(&mut |_| {}).unwrap();
        assert!(!target.binary.exists());
        assert!(!target.entry.exists());
        assert_eq!(target.installed().unwrap(), None);
        // Removing twice is not an error: the job is to leave nothing behind.
        target.remove(&mut |_| {}).unwrap();
    }

    #[test]
    fn a_desktop_entry_written_by_something_else_is_left_alone() {
        let scratch = Scratch::new().unwrap();
        let home = scratch.path.join("home");
        let target = InstallTarget::under(&home, "org.example.Example");

        create_parent(&target.entry).unwrap();
        fs::write(&target.entry, "[Desktop Entry]\nName=Not ours\nExec=/usr/bin/x\n").unwrap();
        fs::create_dir_all(target.binary.parent().unwrap()).unwrap();
        fs::write(&target.binary, b"x").unwrap();

        // No `X-AppImage-Source`, so this is somebody else's entry that happens
        // to share the name — not an integration of ours to report or undo.
        assert_eq!(target.installed().unwrap(), None);
    }

    /// An XPM icon goes into the theme as a PNG of the same dimensions, filed
    /// in the size directory those dimensions call for.
    #[test]
    fn an_xpm_icon_is_converted_before_it_is_integrated() {
        let scratch = Scratch::new().unwrap();
        let target = InstallTarget::under(&scratch.path.join("home"), "org.example.Xpm");

        let source = "/* XPM */\nstatic char *x[] = {\n\"2 2 2 1\",\n\
                      \". 	c #FF0000\",\n\"  	c None\",\n\". \",\n\" .\",\n};\n";
        let name = target
            .write_icon("veracrypt.xpm", source.as_bytes())
            .unwrap();
        assert_eq!(name.as_deref(), Some("org.example.Xpm"));

        let written = target.icon_root.join("2x2/apps/org.example.Xpm.png");
        let bytes = fs::read(&written).expect("the icon lands as a PNG in its size directory");
        assert!(bytes.starts_with(&png::SIGNATURE));
        assert_eq!(png::width(&bytes), Some(2));
        // And removal knows to look for it.
        target.remove(&mut |_| {}).unwrap();
        assert!(!written.exists());
    }

    #[test]
    fn an_icon_nothing_can_decode_is_not_integrated() {
        let scratch = Scratch::new().unwrap();
        let target = InstallTarget::under(&scratch.path.join("home"), "org.example.Bad");

        let name = target.write_icon("icon.ico", &[0u8; 64]).unwrap();
        assert_eq!(name, None);
        assert!(!target.icon_root.exists());
    }
}
