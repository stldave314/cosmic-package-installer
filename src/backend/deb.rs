// SPDX-License-Identifier: GPL-3.0

//! Debian package (`.deb`) support.
//!
//! Inspection is done with `dpkg-deb`, which reads the archive without touching
//! the system. Everything that needs to know about the *system* — is this
//! installed, can these dependencies be satisfied, what would an install
//! actually pull in — goes through `dpkg-query` and `apt`.
//!
//! The dependency view is built from two sources, because neither alone tells
//! the whole truth. The package's own `Depends`/`Recommends` fields say what it
//! asks for, including alternatives the resolver may or may not pick; a single
//! `apt-get --simulate` says what would really happen, including transitive
//! pulls the control fields never mention. The UI shows both.

use std::{collections::HashMap, path::Path};

use cosmic::widget::icon;

use super::{
    exec::{self, Stream},
    Action, Availability, Backend, Dependency, DependencyAlternative, DependencyKind,
    DependencyStatus, Error, InstalledState, OperationPlan, PackageDetails, PackageFormat,
    PayloadEntry, PlannedChange, PlannedChangeKind, Progress, Result,
};
use crate::constants::{
    DEB_APT_TOOL, DEB_CACHE_TOOL, DEB_COMPARE_TOOL, DEB_INSPECT_TOOL, DEB_QUERY_TOOL,
    ICON_EXTENSIONS, ICON_SEARCH_DIRS, INSPECT_TIMEOUT, RESOLVE_TIMEOUT,
};
use crate::debug::DEB;
use crate::debug_log;

/// `Installed-Size` in a Debian control file is in kibibytes, not bytes.
///
/// Getting this wrong understates every package by a factor of 1024, which is
/// just plausible enough not to be noticed.
const INSTALLED_SIZE_UNIT: u64 = 1024;

/// Programs without which this backend cannot function at all.
///
/// `dpkg-deb` alone is enough to *read* a package. The rest are needed to say
/// anything about the system, and a machine with `dpkg-deb` but no `apt` is not
/// one where installing a `.deb` makes sense.
const REQUIRED_TOOLS: &[&str] = &[DEB_INSPECT_TOOL, DEB_QUERY_TOOL, DEB_APT_TOOL];

#[derive(Debug, Default)]
pub struct DebBackend;

impl DebBackend {
    pub fn new() -> Self {
        Self
    }
}

impl Backend for DebBackend {
    fn availability(&self) -> Availability {
        Availability::from_required(REQUIRED_TOOLS)
    }

    fn inspect(&self, path: &Path, include_payload: bool) -> Result<PackageDetails> {
        let path_str = path.to_string_lossy().into_owned();

        let output = exec::run(
            DEB_INSPECT_TOOL,
            &["--field".as_ref(), path.as_os_str()],
            INSPECT_TIMEOUT,
        )?;
        if !output.success() {
            return Err(Error::CommandFailed {
                program: DEB_INSPECT_TOOL.to_string(),
                message: output.failure_message(),
            });
        }

        let mut fields = parse_control(&output.stdout);
        debug_log!(DEB, "{path_str}: {} control fields", fields.len());

        let name = fields
            .remove("package")
            .ok_or_else(|| Error::Parse {
                detail: "control file has no Package field".to_string(),
            })?;
        let version = fields.remove("version").ok_or_else(|| Error::Parse {
            detail: "control file has no Version field".to_string(),
        })?;

        let mut details = PackageDetails::new(PackageFormat::Deb, path, name, version);

        let (summary, description) = fields
            .remove("description")
            .map(|value| split_description(&value))
            .unwrap_or((None, None));
        details.summary = summary;
        details.description = description;

        details.architecture = fields.remove("architecture");
        details.maintainer = fields.remove("maintainer");
        details.homepage = fields.remove("homepage");
        details.section = fields.remove("section");
        details.installed_size = fields
            .remove("installed-size")
            .and_then(|value| value.trim().parse::<u64>().ok())
            .map(|kib| kib * INSTALLED_SIZE_UNIT);

        details.dependencies = parse_dependency_fields(&mut fields);

        // Whatever is left is still worth showing — `Origin`, `Bugs`,
        // `Built-Using` and friends are exactly the kind of thing someone
        // opens a package inspector to look at.
        let mut extra: Vec<(String, String)> = fields
            .into_iter()
            .map(|(key, value)| (title_case_field(&key), value))
            .collect();
        extra.sort_by(|a, b| a.0.cmp(&b.0));
        details.extra = extra;

        // The payload listing is also what icon extraction searches, so it is
        // read even when the file list itself is not being shown.
        let entries = list_contents(path)?;
        details.icon = extract_icon(path, &entries, &details.id);
        if let Some(desktop_name) = desktop_entry_name(path, &entries) {
            details.name = desktop_name;
        }
        if include_payload {
            details.payload = entries.into_iter().map(|(_, entry)| entry).collect();
        }

        Ok(details)
    }

    fn installed_state(&self, details: &PackageDetails) -> Result<InstalledState> {
        let output = exec::run(
            DEB_QUERY_TOOL,
            &[
                "-W",
                "-f=${db:Status-Status}\\t${Version}\\n",
                details.id.as_str(),
            ],
            INSPECT_TIMEOUT,
        )?;

        // `dpkg-query` exits 1 for "no packages matched", which is the ordinary
        // case for something not yet installed. Any other failure means the
        // question went unanswered — dpkg missing, its database locked — and
        // reporting that as "not installed" would invite the user to install
        // something that may already be there.
        match output.code {
            Some(0) => {}
            Some(1) => {
                debug_log!(DEB, "{} is not known to dpkg", details.id);
                return Ok(InstalledState::NotInstalled);
            }
            other => {
                debug_log!(
                    DEB,
                    "dpkg-query failed for {} with {other:?}: {}",
                    details.id,
                    output.failure_message()
                );
                return Ok(InstalledState::Unknown);
            }
        }

        let Some((status, installed)) = output
            .stdout
            .lines()
            .find_map(|line| line.split_once('\t'))
            .map(|(status, version)| (status.trim().to_string(), version.trim().to_string()))
        else {
            return Ok(InstalledState::NotInstalled);
        };

        // "config-files" means the package was removed but not purged; nothing
        // of it is actually on the system, so treat it as not installed.
        if status != "installed" || installed.is_empty() {
            debug_log!(DEB, "{} status is {status:?}", details.id);
            return Ok(InstalledState::NotInstalled);
        }

        Ok(match compare_versions(&details.version, &installed)? {
            std::cmp::Ordering::Equal => InstalledState::SameVersion { installed },
            std::cmp::Ordering::Greater => InstalledState::Older { installed },
            std::cmp::Ordering::Less => InstalledState::Newer { installed },
        })
    }

    fn resolve_dependencies(&self, details: &mut PackageDetails) -> Result<()> {
        // One apt-cache call for every name mentioned anywhere, rather than one
        // per dependency: a desktop application can declare well over a hundred,
        // and the per-process cost dominates everything else here.
        let mut names: Vec<String> = Vec::new();
        for dependency in &details.dependencies {
            for alternative in &dependency.alternatives {
                if !names.contains(&alternative.name) {
                    names.push(alternative.name.clone());
                }
            }
        }
        if names.is_empty() {
            return Ok(());
        }
        debug_log!(DEB, "resolving {} dependency names", names.len());

        let policies = apt_cache_policy(&names)?;

        // Names apt knows nothing about may still be virtual — provided by some
        // other installable package. Only those are worth a second lookup.
        let unresolved: Vec<String> = names
            .iter()
            .filter(|name| {
                policies
                    .get(*name)
                    .is_none_or(|policy| policy.installed.is_none() && policy.candidate.is_none())
            })
            .cloned()
            .collect();
        let providers = if unresolved.is_empty() {
            HashMap::new()
        } else {
            debug_log!(DEB, "checking {} names for providers", unresolved.len());
            apt_cache_providers(&unresolved).unwrap_or_default()
        };

        for dependency in &mut details.dependencies {
            for alternative in &mut dependency.alternatives {
                alternative.status = match policies.get(&alternative.name) {
                    Some(policy) => match (&policy.installed, &policy.candidate) {
                        (Some(installed), _) => DependencyStatus::Installed {
                            version: installed.clone(),
                        },
                        (None, Some(candidate)) => DependencyStatus::Available {
                            version: candidate.clone(),
                        },
                        (None, None) => provided_or_missing(&providers, &alternative.name),
                    },
                    None => provided_or_missing(&providers, &alternative.name),
                };
            }
        }

        Ok(())
    }

    fn plan(&self, details: &PackageDetails, action: Action) -> Result<OperationPlan> {
        let args = apt_args(details, action, true);
        let output = exec::run(DEB_APT_TOOL, &args, RESOLVE_TIMEOUT)?;

        let mut plan = parse_apt_simulation(&output.stdout);

        if !output.success() {
            // apt explains unmet dependencies far better than any message this
            // application could synthesise, so its own words are carried
            // through to the UI verbatim.
            plan.blocked = Some(output.failure_message());
            debug_log!(DEB, "plan blocked: {:?}", plan.blocked);
        }

        // Upgrade or downgrade is decided by comparing versions, which only
        // dpkg can do correctly (epochs, tildes, and `~rc1` sorting before the
        // release it precedes).
        for change in &mut plan.changes {
            if change.kind == PlannedChangeKind::Upgrade {
                if let (Some(new), Some(current)) = (&change.version, &change.current_version) {
                    if compare_versions(new, current)? == std::cmp::Ordering::Less {
                        change.kind = PlannedChangeKind::Downgrade;
                    }
                }
            }
        }

        compute_sizes(&mut plan, details);
        debug_log!(
            DEB,
            "plan: {} changes, download {:?}, disk {:?}",
            plan.changes.len(),
            plan.download_size,
            plan.disk_size_delta
        );

        Ok(plan)
    }

    fn perform(
        &self,
        details: &PackageDetails,
        action: Action,
        on_progress: &mut dyn FnMut(Progress),
    ) -> Result<()> {
        super::privileged::perform_deb(details, action, on_progress)
    }
}

/// The `apt-get` arguments for `action`, either simulating or performing it.
pub fn apt_args(details: &PackageDetails, action: Action, simulate: bool) -> Vec<String> {
    let mut args = Vec::new();
    args.push(
        if action == Action::Remove {
            "remove"
        } else {
            "install"
        }
        .to_string(),
    );

    if simulate {
        args.push("--simulate".to_string());
    } else {
        args.push("--assume-yes".to_string());
    }

    match action {
        // Without this apt reports "already the newest version" and does
        // nothing at all, which looks to the user like the button is broken.
        Action::Reinstall => args.push("--reinstall".to_string()),
        // apt refuses to go backwards unless explicitly told it may.
        Action::Downgrade => args.push("--allow-downgrades".to_string()),
        _ => {}
    }

    if action == Action::Remove {
        args.push(details.id.clone());
    } else {
        // apt treats an argument without a slash as a package name, so the
        // path must stay absolute for a local file to be recognised.
        args.push(details.path.clone());
    }
    args
}

/// Classify a name apt has no version for.
fn provided_or_missing(
    providers: &HashMap<String, Vec<String>>,
    name: &str,
) -> DependencyStatus {
    match providers.get(name) {
        Some(list) if !list.is_empty() => DependencyStatus::ProvidedBy {
            providers: list.clone(),
        },
        _ => DependencyStatus::Missing,
    }
}

// ── Control file parsing ────────────────────────────────────────────────────

/// Parse a Debian control stanza into lower-cased field names and their values.
///
/// Field names are case-insensitive per policy, so they are normalised here and
/// every lookup can use one spelling. Continuation lines (those starting with
/// whitespace) are joined onto the preceding field with their leading space
/// preserved, because `Description` depends on that indentation for meaning.
fn parse_control(text: &str) -> HashMap<String, String> {
    let mut fields: HashMap<String, String> = HashMap::new();
    let mut current: Option<String> = None;

    for line in text.lines() {
        if line.starts_with(' ') || line.starts_with('\t') {
            if let Some(key) = &current {
                if let Some(value) = fields.get_mut(key) {
                    value.push('\n');
                    value.push_str(line);
                }
            }
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let key = key.trim().to_ascii_lowercase();
        fields.insert(key.clone(), value.trim().to_string());
        current = Some(key);
    }

    fields
}

/// Split a `Description` field into its one-line summary and the extended part.
///
/// The extended description uses a lone `.` on an otherwise blank line to mean
/// a paragraph break, and a leading space on every line as its indent marker.
fn split_description(value: &str) -> (Option<String>, Option<String>) {
    let mut lines = value.lines();
    let summary = lines.next().map(|line| line.trim().to_string());

    let body = lines
        .map(|line| {
            // Every continuation line carries one space of indent as its
            // marker; anything beyond that is the package's own formatting and
            // is kept.
            let stripped = line.strip_prefix(' ').unwrap_or(line);
            if stripped.trim() == "." {
                // A lone `.` is a paragraph break, which becomes a blank line
                // once the lines are joined.
                String::new()
            } else {
                stripped.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();
    (
        summary.filter(|s| !s.is_empty()),
        Some(body).filter(|s| !s.is_empty()),
    )
}

/// Restore a lower-cased control field name to its conventional spelling.
///
/// Only used for the leftover fields shown verbatim, so `built-using` reads as
/// `Built-Using` rather than being displayed the way it was normalised.
fn title_case_field(key: &str) -> String {
    key.split('-')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join("-")
}

// ── Dependency parsing ──────────────────────────────────────────────────────

/// Control fields that describe relationships, and the kind each maps to.
const RELATIONSHIP_FIELDS: &[(&str, DependencyKind)] = &[
    ("pre-depends", DependencyKind::PreDepends),
    ("depends", DependencyKind::Depends),
    ("recommends", DependencyKind::Recommends),
    ("suggests", DependencyKind::Suggests),
    ("conflicts", DependencyKind::Conflicts),
    ("breaks", DependencyKind::Breaks),
    ("replaces", DependencyKind::Replaces),
    ("provides", DependencyKind::Provides),
];

/// Pull every relationship field out of `fields`, leaving the rest behind.
fn parse_dependency_fields(fields: &mut HashMap<String, String>) -> Vec<Dependency> {
    let mut dependencies = Vec::new();
    for (field, kind) in RELATIONSHIP_FIELDS {
        let Some(value) = fields.remove(*field) else {
            continue;
        };
        dependencies.extend(parse_dependency_list(&value, *kind));
    }
    dependencies
}

/// Parse one relationship field, e.g. `libc6 (>= 2.34), exim4 | mail-transport-agent`.
fn parse_dependency_list(value: &str, kind: DependencyKind) -> Vec<Dependency> {
    value
        .split(',')
        .filter_map(|group| {
            let alternatives: Vec<DependencyAlternative> =
                group.split('|').filter_map(parse_alternative).collect();
            if alternatives.is_empty() {
                return None;
            }
            Some(Dependency { kind, alternatives })
        })
        .collect()
}

/// Parse a single alternative, e.g. `libc6:any (>= 2.34) [amd64] <!nocheck>`.
fn parse_alternative(text: &str) -> Option<DependencyAlternative> {
    // Architecture restrictions and build profiles never affect whether the
    // dependency is satisfiable on the machine in front of us, and only get in
    // the way of reading the name.
    let text = strip_bracketed(text, '[', ']');
    let text = strip_bracketed(&text, '<', '>');

    let (name_part, constraint) = match text.find('(') {
        Some(open) => {
            let close = text[open..].find(')').map(|offset| open + offset);
            match close {
                Some(close) => (
                    text[..open].to_string(),
                    Some(normalize_whitespace(&text[open + 1..close])),
                ),
                // Unbalanced parenthesis: take the name and drop the rest
                // rather than discarding an otherwise valid dependency.
                None => (text[..open].to_string(), None),
            }
        }
        None => (text.clone(), None),
    };

    // Multi-arch qualifiers (`libfoo:any`, `libfoo:amd64`) are not part of the
    // package name apt answers to.
    let name = name_part
        .trim()
        .split(':')
        .next()
        .unwrap_or("")
        .trim()
        .to_string();
    if name.is_empty() {
        return None;
    }

    Some(DependencyAlternative {
        name,
        constraint: constraint.filter(|c| !c.is_empty()),
        status: DependencyStatus::Unknown,
    })
}

/// Remove every `open`…`close` span from `text`.
///
/// An unmatched closing character is kept rather than dropped. That matters
/// more than it looks: this is used to strip build profiles like `<!nocheck>`,
/// and a version constraint such as `(>= 2.34)` contains a `>` that never
/// opened anything. Treating it as a stray terminator silently turns the
/// constraint into `= 2.34`.
fn strip_bracketed(text: &str, open: char, close: char) -> String {
    let mut result = String::with_capacity(text.len());
    let mut depth = 0usize;
    for character in text.chars() {
        if character == open {
            depth += 1;
        } else if character == close && depth > 0 {
            depth -= 1;
        } else if depth == 0 {
            result.push(character);
        }
    }
    result
}

/// Collapse runs of whitespace to single spaces and trim the ends.
fn normalize_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ── Payload listing ─────────────────────────────────────────────────────────

/// List the package payload, pairing each entry with its name inside the
/// archive.
///
/// The archive name is kept because extraction has to ask `tar` for the member
/// by its exact name, which is usually `./usr/...` — not the absolute path the
/// entry will occupy once installed.
fn list_contents(path: &Path) -> Result<Vec<(String, PayloadEntry)>> {
    let output = exec::run(
        DEB_INSPECT_TOOL,
        &["--contents".as_ref(), path.as_os_str()],
        INSPECT_TIMEOUT,
    )?;
    if !output.success() {
        return Err(Error::CommandFailed {
            program: DEB_INSPECT_TOOL.to_string(),
            message: output.failure_message(),
        });
    }
    let entries: Vec<(String, PayloadEntry)> =
        output.stdout.lines().filter_map(parse_contents_line).collect();
    debug_log!(DEB, "payload has {} entries", entries.len());
    Ok(entries)
}

/// Parse one line of `dpkg-deb --contents` output.
///
/// The format is `tar -tv` style: mode, owner/group, size, date, time, then the
/// name — which may itself contain spaces, and for a symlink is followed by
/// ` -> target`.
fn parse_contents_line(line: &str) -> Option<(String, PayloadEntry)> {
    let mut rest = line;
    let mut fields: Vec<&str> = Vec::with_capacity(5);
    for _ in 0..5 {
        rest = rest.trim_start();
        let end = rest.find(char::is_whitespace)?;
        fields.push(&rest[..end]);
        rest = &rest[end..];
    }

    // Exactly one space separates the time from the name. Consuming just that
    // one keeps a name that legitimately begins with a space intact.
    let name = rest.strip_prefix(' ').unwrap_or_else(|| rest.trim_start());
    if name.is_empty() {
        return None;
    }

    let mode = fields[0];
    let (name, link_target) = match name.split_once(" -> ") {
        Some((name, target)) if mode.starts_with('l') => (name, Some(target.to_string())),
        _ => (name, None),
    };

    let archive_name = name.to_string();
    let path = normalize_payload_path(name);
    // The archive root adds nothing to a file list.
    if path == "/" {
        return None;
    }

    Some((
        archive_name,
        PayloadEntry {
            path,
            link_target,
            is_directory: mode.starts_with('d'),
            size: fields[2].parse::<u64>().ok(),
        },
    ))
}

/// Turn an archive member name into the absolute path it will occupy.
fn normalize_payload_path(name: &str) -> String {
    let trimmed = name.strip_prefix("./").unwrap_or(name);
    let trimmed = trimmed.strip_prefix('/').unwrap_or(trimmed);
    let trimmed = trimmed.trim_end_matches('/');
    format!("/{trimmed}")
}

// ── Icon and name extraction ────────────────────────────────────────────────

/// Read a single file out of the package payload.
fn extract_file(path: &Path, archive_name: &str) -> Option<Vec<u8>> {
    // Most packages name members `./usr/...`, a few use `usr/...`. Asking for
    // both costs nothing: tar extracts whichever exists and complains about the
    // other on stderr, which is discarded.
    let alternate = archive_name
        .strip_prefix("./")
        .map(str::to_string)
        .unwrap_or_else(|| format!("./{}", archive_name.trim_start_matches('/')));

    let bytes = exec::run_piped(
        DEB_INSPECT_TOOL,
        &["--fsys-tarfile".as_ref(), path.as_os_str()],
        "tar",
        &["-xO", archive_name, alternate.as_str()],
        INSPECT_TIMEOUT,
    )
    .ok()?;

    if bytes.is_empty() {
        None
    } else {
        Some(bytes)
    }
}

/// Find the package's desktop entry, if it ships one.
fn find_desktop_entry(entries: &[(String, PayloadEntry)]) -> Option<&str> {
    entries
        .iter()
        .find(|(_, entry)| {
            !entry.is_directory
                && entry.path.starts_with("/usr/share/applications/")
                && entry.path.ends_with(".desktop")
        })
        .map(|(archive_name, _)| archive_name.as_str())
}

/// The application's display name, taken from its desktop entry.
///
/// A package name like `firefox-esr` is not what the user calls the thing they
/// are installing, and the desktop entry is where the real name lives.
fn desktop_entry_name(path: &Path, entries: &[(String, PayloadEntry)]) -> Option<String> {
    let archive_name = find_desktop_entry(entries)?;
    let bytes = extract_file(path, archive_name)?;
    let text = String::from_utf8_lossy(&bytes);
    desktop_field(&text, "Name")
}

/// Read an unlocalised field from the `[Desktop Entry]` group.
///
/// Localised variants (`Name[de]`) are skipped deliberately: matching them
/// against the user's locale properly is the desktop-entry spec's job, and
/// getting it half-right would show a German name to a French user.
fn desktop_field(text: &str, field: &str) -> Option<String> {
    let mut in_entry_group = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_entry_group = line == "[Desktop Entry]";
            continue;
        }
        if !in_entry_group {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim() == field {
            let value = value.trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

/// Pull an application icon out of the package.
///
/// Tries the icon named by the desktop entry first, then one named after the
/// package, then anything in the standard icon directories — in that order
/// because a package may ship several icons and only one of them is the
/// application's.
fn extract_icon(
    path: &Path,
    entries: &[(String, PayloadEntry)],
    package_name: &str,
) -> Option<icon::Handle> {
    let desktop_icon = find_desktop_entry(entries)
        .and_then(|archive_name| extract_file(path, archive_name))
        .and_then(|bytes| desktop_field(&String::from_utf8_lossy(&bytes), "Icon"));

    let mut candidates: Vec<&str> = Vec::new();

    // An `Icon=` that is already an absolute path names the file directly.
    if let Some(name) = desktop_icon.as_deref() {
        if name.starts_with('/') {
            if let Some((archive_name, _)) =
                entries.iter().find(|(_, entry)| entry.path == name)
            {
                candidates.push(archive_name);
            }
        }
    }

    for stem in desktop_icon.as_deref().into_iter().chain([package_name]) {
        if stem.starts_with('/') {
            continue;
        }
        for directory in ICON_SEARCH_DIRS {
            for extension in ICON_EXTENSIONS {
                let wanted = format!("/{directory}{stem}.{extension}");
                if let Some((archive_name, _)) =
                    entries.iter().find(|(_, entry)| entry.path == wanted)
                {
                    if !candidates.contains(&archive_name.as_str()) {
                        candidates.push(archive_name);
                    }
                }
            }
        }
    }

    // Last resort: any renderable icon in a standard location, preferring the
    // directories in their listed order.
    if candidates.is_empty() {
        for directory in ICON_SEARCH_DIRS {
            let prefix = format!("/{directory}");
            if let Some((archive_name, _)) = entries.iter().find(|(_, entry)| {
                entry.path.starts_with(&prefix)
                    && ICON_EXTENSIONS.iter().any(|extension| {
                        entry
                            .path
                            .to_ascii_lowercase()
                            .ends_with(&format!(".{extension}"))
                    })
            }) {
                candidates.push(archive_name);
                break;
            }
        }
    }

    for archive_name in candidates {
        let Some(bytes) = extract_file(path, archive_name) else {
            continue;
        };
        debug_log!(
            crate::debug::ICON,
            "extracted {archive_name} ({} bytes)",
            bytes.len()
        );
        if archive_name.to_ascii_lowercase().ends_with(".svg") {
            return Some(icon::from_svg_bytes(bytes));
        }
        return Some(icon::from_raster_bytes(bytes));
    }

    debug_log!(crate::debug::ICON, "no icon found in {}", path.display());
    None
}

// ── System queries ──────────────────────────────────────────────────────────

/// Compare two Debian versions, ordering `left` relative to `right`.
///
/// Delegates to `dpkg` rather than reimplementing the comparison. Debian
/// versions have epochs, tildes that sort *before* the empty string, and
/// digit/non-digit alternation rules; a hand-rolled comparison that is subtly
/// wrong would silently mislabel upgrades as downgrades.
fn compare_versions(left: &str, right: &str) -> Result<std::cmp::Ordering> {
    for (operator, ordering) in [
        ("eq", std::cmp::Ordering::Equal),
        ("gt", std::cmp::Ordering::Greater),
    ] {
        let output = exec::run(
            DEB_COMPARE_TOOL,
            &["--compare-versions", left, operator, right],
            INSPECT_TIMEOUT,
        )?;
        if output.success() {
            return Ok(ordering);
        }
    }
    Ok(std::cmp::Ordering::Less)
}

/// What apt knows about one package name.
#[derive(Clone, Debug, Default)]
struct AptPolicy {
    installed: Option<String>,
    candidate: Option<String>,
}

/// Run `apt-cache policy` over `names` and parse the per-package answer.
///
/// One call covers both questions the dependency list asks — is it installed,
/// and can it be installed — because apt reports the installed version and the
/// candidate version together.
fn apt_cache_policy(names: &[String]) -> Result<HashMap<String, AptPolicy>> {
    let output = exec::run(DEB_CACHE_TOOL, &prefixed("policy", names), RESOLVE_TIMEOUT)?;

    // Names apt has never heard of produce a warning and are simply absent
    // from the output, which is a non-zero exit but not a failure for us.
    let mut policies: HashMap<String, AptPolicy> = HashMap::new();
    let mut current: Option<String> = None;

    for line in output.stdout.lines() {
        if !line.starts_with(char::is_whitespace) {
            // A package header looks like `libc6:` at column zero.
            if let Some(name) = line.trim().strip_suffix(':') {
                // Strip any architecture qualifier apt echoed back.
                let name = name.split(':').next().unwrap_or(name).to_string();
                policies.entry(name.clone()).or_default();
                current = Some(name);
            } else {
                current = None;
            }
            continue;
        }

        let Some(name) = &current else { continue };
        let trimmed = line.trim();
        let Some((key, value)) = trimmed.split_once(':') else {
            continue;
        };
        let value = value.trim();
        // apt writes `(none)` for "no such version".
        let value = if value == "(none)" || value.is_empty() {
            None
        } else {
            Some(value.to_string())
        };

        if let Some(policy) = policies.get_mut(name) {
            match key {
                "Installed" => policy.installed = value,
                "Candidate" => policy.candidate = value,
                _ => {}
            }
        }
    }

    debug_log!(DEB, "apt-cache policy resolved {} names", policies.len());
    Ok(policies)
}

/// Find installable packages that provide each of `names`.
///
/// Answers the question `apt-cache policy` cannot: a name with no versions may
/// still be a virtual package, in which case the dependency is satisfiable even
/// though nothing by that name exists.
fn apt_cache_providers(names: &[String]) -> Result<HashMap<String, Vec<String>>> {
    let output = exec::run(DEB_CACHE_TOOL, &prefixed("showpkg", names), RESOLVE_TIMEOUT)?;

    let mut providers: HashMap<String, Vec<String>> = HashMap::new();
    let mut current: Option<String> = None;
    let mut in_reverse_provides = false;

    for line in output.stdout.lines() {
        if let Some(name) = line.strip_prefix("Package: ") {
            current = Some(name.trim().to_string());
            in_reverse_provides = false;
            continue;
        }
        // Every section header sits at column zero and ends with a colon, so
        // any of them closes the Reverse Provides list.
        if !line.starts_with(char::is_whitespace) && line.trim_end().ends_with(':') {
            in_reverse_provides = line.trim_end() == "Reverse Provides:";
            continue;
        }
        if !in_reverse_provides {
            continue;
        }
        let Some(name) = &current else { continue };
        // Entries read `provider-package 1.2.3`.
        if let Some(provider) = line.split_whitespace().next() {
            providers
                .entry(name.clone())
                .or_default()
                .push(provider.to_string());
        }
    }

    Ok(providers)
}

/// Build an argument list of `subcommand` followed by `names`.
fn prefixed(subcommand: &str, names: &[String]) -> Vec<String> {
    let mut args = Vec::with_capacity(names.len() + 1);
    args.push(subcommand.to_string());
    args.extend_from_slice(names);
    args
}

// ── Simulation parsing ──────────────────────────────────────────────────────

/// Parse the output of `apt-get --simulate`.
///
/// Only the `Inst` and `Remv` lines are of interest, and they are the reason
/// this approach works at all: they are apt's machine-readable summary and have
/// been stable for many years.
///
/// Notably absent is any attempt to read the "Need to get …" and "After this
/// operation …" lines. `apt-get --simulate` does not print them — they belong
/// to the confirmation prompt, which a simulation never reaches. Sizes are
/// computed separately by [`compute_sizes`].
fn parse_apt_simulation(text: &str) -> OperationPlan {
    let mut plan = OperationPlan::default();

    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("Inst ") {
            if let Some(change) = parse_inst_line(rest) {
                plan.changes.push(change);
            }
        } else if let Some(rest) = line.strip_prefix("Remv ") {
            if let Some(change) = parse_remv_line(rest) {
                plan.changes.push(change);
            }
        }
    }

    plan
}

/// Work out the download and disk-space figures for a plan.
///
/// These are derived from package metadata rather than scraped from apt's
/// human-readable summary: `apt-cache show` reports an exact `Size` and
/// `Installed-Size` per version, and `dpkg-query` reports the size of what is
/// already installed. Arithmetic on those numbers cannot be broken by a change
/// of wording or of locale.
///
/// Failures here are deliberately swallowed. A missing size means the UI omits
/// one line; it is not a reason to refuse to show the user what will be
/// installed.
fn compute_sizes(plan: &mut OperationPlan, details: &PackageDetails) {
    // The package being installed comes from a local file, so its own metadata
    // is authoritative and it needs no download.
    let mut download: u64 = 0;
    let mut delta: i64 = 0;
    let mut known = false;

    let specs: Vec<String> = plan
        .changes
        .iter()
        .filter(|change| change.kind != PlannedChangeKind::Remove && change.name != details.id)
        .filter_map(|change| {
            change
                .version
                .as_ref()
                .map(|version| format!("{}={}", change.name, version))
        })
        .collect();

    let candidate_sizes = if specs.is_empty() {
        HashMap::new()
    } else {
        apt_cache_sizes(&specs).unwrap_or_default()
    };

    let current_names: Vec<String> = plan
        .changes
        .iter()
        .filter(|change| change.current_version.is_some())
        .map(|change| change.name.clone())
        .collect();
    let current_sizes = if current_names.is_empty() {
        HashMap::new()
    } else {
        dpkg_installed_sizes(&current_names).unwrap_or_default()
    };

    for change in &plan.changes {
        let is_local = change.name == details.id;

        let new_size = if is_local {
            details.installed_size
        } else {
            candidate_sizes
                .get(&change.name)
                .and_then(|sizes| sizes.installed_size)
        };
        let old_size = current_sizes.get(&change.name).copied();

        match change.kind {
            PlannedChangeKind::Install => {
                if let Some(size) = new_size {
                    delta += size as i64;
                    known = true;
                }
            }
            PlannedChangeKind::Upgrade | PlannedChangeKind::Downgrade => {
                if let Some(size) = new_size {
                    delta += size as i64;
                    known = true;
                }
                if let Some(size) = old_size {
                    delta -= size as i64;
                    known = true;
                }
            }
            PlannedChangeKind::Remove => {
                if let Some(size) = old_size {
                    delta -= size as i64;
                    known = true;
                }
            }
        }

        // A local file is already on disk; only packages apt has to fetch
        // count towards the download.
        if !is_local && change.kind != PlannedChangeKind::Remove {
            if let Some(size) = candidate_sizes
                .get(&change.name)
                .and_then(|sizes| sizes.download_size)
            {
                download += size;
            }
        }
    }

    plan.download_size = Some(download);
    plan.disk_size_delta = known.then_some(delta);
}

/// Sizes apt knows for one package version.
#[derive(Clone, Copy, Debug, Default)]
struct CandidateSizes {
    /// Bytes to download.
    download_size: Option<u64>,
    /// Bytes occupied once unpacked.
    installed_size: Option<u64>,
}

/// Read `Size` and `Installed-Size` for each `name=version` spec.
fn apt_cache_sizes(specs: &[String]) -> Result<HashMap<String, CandidateSizes>> {
    let output = exec::run(DEB_CACHE_TOOL, &prefixed("show", specs), RESOLVE_TIMEOUT)?;

    let mut sizes: HashMap<String, CandidateSizes> = HashMap::new();
    let mut name: Option<String> = None;
    let mut current = CandidateSizes::default();

    let mut flush = |name: &mut Option<String>, current: &mut CandidateSizes| {
        if let Some(name) = name.take() {
            sizes.insert(name, std::mem::take(current));
        }
    };

    for line in output.stdout.lines() {
        // Stanzas are separated by blank lines.
        if line.trim().is_empty() {
            flush(&mut name, &mut current);
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();
        match key {
            "Package" => name = Some(value.to_string()),
            "Size" => current.download_size = value.parse().ok(),
            "Installed-Size" => {
                current.installed_size = value
                    .parse::<u64>()
                    .ok()
                    .map(|kib| kib * INSTALLED_SIZE_UNIT)
            }
            _ => {}
        }
    }
    flush(&mut name, &mut current);

    Ok(sizes)
}

/// Read the on-disk size of each currently-installed package, in bytes.
fn dpkg_installed_sizes(names: &[String]) -> Result<HashMap<String, u64>> {
    let mut args = vec![
        "-W".to_string(),
        "-f=${Package}\\t${Installed-Size}\\n".to_string(),
    ];
    args.extend_from_slice(names);
    let output = exec::run(DEB_QUERY_TOOL, &args, INSPECT_TIMEOUT)?;

    Ok(output
        .stdout
        .lines()
        .filter_map(|line| line.split_once('\t'))
        .filter_map(|(name, size)| {
            size.trim()
                .parse::<u64>()
                .ok()
                .map(|kib| (name.trim().to_string(), kib * INSTALLED_SIZE_UNIT))
        })
        .collect())
}

/// Parse `libfoo [1.0] (1.1 Ubuntu:24.04/noble [amd64])` — the part of an
/// `Inst` line after the keyword. The bracketed version is present only when
/// something is already installed, which is what distinguishes an upgrade from
/// a fresh install.
fn parse_inst_line(rest: &str) -> Option<PlannedChange> {
    let rest = rest.trim();
    let (name, tail) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
    let tail = tail.trim();

    let (current_version, tail) = match tail.strip_prefix('[') {
        Some(inner) => match inner.find(']') {
            Some(end) => (Some(inner[..end].to_string()), inner[end + 1..].trim()),
            None => (None, tail),
        },
        None => (None, tail),
    };

    let version = tail
        .strip_prefix('(')
        .and_then(|inner| inner.split_whitespace().next())
        .map(|version| version.trim_end_matches(')').to_string());

    Some(PlannedChange {
        name: name.to_string(),
        version,
        kind: if current_version.is_some() {
            // Refined to Downgrade by the caller, which can compare versions.
            PlannedChangeKind::Upgrade
        } else {
            PlannedChangeKind::Install
        },
        current_version,
    })
}

/// Parse `libfoo [1.0]` — the part of a `Remv` line after the keyword.
fn parse_remv_line(rest: &str) -> Option<PlannedChange> {
    let rest = rest.trim();
    let (name, tail) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
    let current_version = tail
        .trim()
        .strip_prefix('[')
        .and_then(|inner| inner.split(']').next())
        .map(str::to_string);

    Some(PlannedChange {
        name: name.to_string(),
        version: None,
        current_version,
        kind: PlannedChangeKind::Remove,
    })
}

/// Turn a line of `apt-get` output into a progress report.
///
/// apt has no machine-readable progress on stdout, so the best available signal
/// is the status lines it prints as it works. They are shown to the user as-is.
pub fn progress_from_line(stream: Stream, line: &str) -> Option<Progress> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    // Percentages apt prints for its own progress meter are noise here; the
    // meaningful lines are the ones naming what it is doing.
    if stream == Stream::Stderr && line.starts_with("debconf:") {
        return None;
    }
    Some(Progress::Status(line.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_control_stanza_with_continuations() {
        let fields = parse_control(
            "Package: foo\nVersion: 1:2.3-4\nDescription: a short summary\n a long line\n .\n another\n",
        );
        assert_eq!(fields.get("package").map(String::as_str), Some("foo"));
        assert_eq!(fields.get("version").map(String::as_str), Some("1:2.3-4"));

        let (summary, description) = split_description(fields.get("description").unwrap());
        assert_eq!(summary.as_deref(), Some("a short summary"));
        assert_eq!(description.as_deref(), Some("a long line\n\nanother"));
    }

    #[test]
    fn parses_alternatives_and_constraints() {
        let deps = parse_dependency_list(
            "libc6 (>= 2.34), exim4 | mail-transport-agent, libfoo:any [amd64] <!nocheck>",
            DependencyKind::Depends,
        );
        assert_eq!(deps.len(), 3);
        assert_eq!(deps[0].alternatives[0].name, "libc6");
        assert_eq!(deps[0].alternatives[0].constraint.as_deref(), Some(">= 2.34"));
        assert_eq!(deps[1].alternatives.len(), 2);
        assert_eq!(deps[1].alternatives[1].name, "mail-transport-agent");
        assert_eq!(deps[2].alternatives[0].name, "libfoo");
        assert!(deps[2].alternatives[0].constraint.is_none());
    }

    #[test]
    fn parses_contents_lines_including_symlinks_and_spaces() {
        let (archive, entry) = parse_contents_line(
            "-rw-r--r-- root/root      1234 2024-01-01 00:00 ./usr/share/My App/data.bin",
        )
        .unwrap();
        assert_eq!(archive, "./usr/share/My App/data.bin");
        assert_eq!(entry.path, "/usr/share/My App/data.bin");
        assert_eq!(entry.size, Some(1234));
        assert!(!entry.is_directory);

        let (_, link) = parse_contents_line(
            "lrwxrwxrwx root/root         0 2024-01-01 00:00 ./usr/lib/libfoo.so -> libfoo.so.1",
        )
        .unwrap();
        assert_eq!(link.link_target.as_deref(), Some("libfoo.so.1"));

        // The archive root carries no information and is dropped.
        assert!(parse_contents_line("drwxr-xr-x root/root 0 2024-01-01 00:00 ./").is_none());
    }

    #[test]
    fn parses_apt_simulation() {
        let plan = parse_apt_simulation(
            "Reading package lists...\n\
             The following NEW packages will be installed:\n\
               libnew\n\
             Inst libnew (1.0 Ubuntu:24.04/noble [amd64])\n\
             Inst libold [1.0] (2.0 Ubuntu:24.04/noble, local-deb [amd64])\n\
             Remv obsolete [3.0]\n\
             Conf libnew (1.0 Ubuntu:24.04/noble [amd64])\n",
        );
        assert_eq!(plan.changes.len(), 3);
        assert_eq!(plan.changes[0].kind, PlannedChangeKind::Install);
        assert_eq!(plan.changes[0].name, "libnew");
        assert_eq!(plan.changes[0].version.as_deref(), Some("1.0"));
        assert!(plan.changes[0].current_version.is_none());

        assert_eq!(plan.changes[1].kind, PlannedChangeKind::Upgrade);
        assert_eq!(plan.changes[1].current_version.as_deref(), Some("1.0"));
        assert_eq!(plan.changes[1].version.as_deref(), Some("2.0"));

        assert_eq!(plan.changes[2].kind, PlannedChangeKind::Remove);
        assert_eq!(plan.changes[2].name, "obsolete");
        assert_eq!(plan.changes[2].current_version.as_deref(), Some("3.0"));
    }

    #[test]
    fn parses_apt_cache_show_stanzas() {
        let sizes = {
            // Same shape as `apt-cache show a=1.0 b=2.0`: stanzas separated by
            // blank lines, with `Installed-Size` in kibibytes.
            let text = "Package: hello\nVersion: 2.10\nInstalled-Size: 104\nSize: 26006\n\n\
                        Package: goodbye\nVersion: 1.0\nInstalled-Size: 8\nSize: 512\n";
            let mut sizes: HashMap<String, CandidateSizes> = HashMap::new();
            let mut name: Option<String> = None;
            let mut current = CandidateSizes::default();
            for line in text.lines().chain(std::iter::once("")) {
                if line.trim().is_empty() {
                    if let Some(name) = name.take() {
                        sizes.insert(name, std::mem::take(&mut current));
                    }
                    continue;
                }
                if let Some((key, value)) = line.split_once(':') {
                    let value = value.trim();
                    match key {
                        "Package" => name = Some(value.to_string()),
                        "Size" => current.download_size = value.parse().ok(),
                        "Installed-Size" => {
                            current.installed_size =
                                value.parse::<u64>().ok().map(|kib| kib * INSTALLED_SIZE_UNIT)
                        }
                        _ => {}
                    }
                }
            }
            sizes
        };

        assert_eq!(sizes["hello"].download_size, Some(26_006));
        assert_eq!(sizes["hello"].installed_size, Some(104 * 1024));
        assert_eq!(sizes["goodbye"].installed_size, Some(8 * 1024));
    }

    #[test]
    fn reads_unlocalised_desktop_fields_only() {
        let text = "[Desktop Entry]\nName=Real Name\nName[de]=Deutscher Name\nIcon=my-icon\n\n[Other]\nName=Wrong\n";
        assert_eq!(desktop_field(text, "Name").as_deref(), Some("Real Name"));
        assert_eq!(desktop_field(text, "Icon").as_deref(), Some("my-icon"));
    }
}
