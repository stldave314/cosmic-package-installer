// SPDX-License-Identifier: GPL-3.0

//! Reading and rewriting desktop-entry files.
//!
//! Three backends want the same two things from a `.desktop` file — the name
//! the user actually calls the application, and the icon it declares — so the
//! parsing lives here rather than three times over.
//!
//! The same syntax turns up beyond `.desktop` files: a Flatpak's `metadata` and
//! a `.flatpakref` are both grouped key/value files of exactly this shape, so
//! [`field_in`] and [`fields_in`] take the group as an argument and the
//! Flatpak backend reads its own files with them.
//!
//! Rewriting is only needed by the AppImage backend, which cannot install a
//! bundled desktop entry unchanged: its `Exec` names a command that exists
//! solely inside the AppImage's own mount, and its `Icon` names an icon that
//! has to be given a unique name before it goes into the shared icon theme.

/// The group an application's own fields live in. Fields in other groups —
/// action definitions, in particular — are not the application's own.
const ENTRY_GROUP: &str = "[Desktop Entry]";

/// Read an unlocalised field from the `[Desktop Entry]` group.
pub fn field(text: &str, key: &str) -> Option<String> {
    field_in(text, ENTRY_GROUP, key)
}

/// Read an unlocalised field from an arbitrary group, which must be given with
/// its brackets, e.g. `[Application]`.
///
/// Localised variants (`Name[de]`) are skipped deliberately: matching them
/// against the user's locale properly is the desktop-entry spec's job, and
/// getting it half-right would show a German name to a French user.
pub fn field_in(text: &str, group: &str, key: &str) -> Option<String> {
    fields_in(text, group)
        .into_iter()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value)
}

/// Every non-empty key and value in `group`, in file order.
pub fn fields_in(text: &str, group: &str) -> Vec<(String, String)> {
    let mut fields = Vec::new();
    let mut in_group = false;

    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_group = line == group;
            continue;
        }
        if !in_group || line.starts_with('#') {
            continue;
        }
        let Some((name, value)) = line.split_once('=') else {
            continue;
        };
        let (name, value) = (name.trim(), value.trim());
        // A key with a locale qualifier is a translation of one already listed.
        if name.is_empty() || value.is_empty() || name.contains('[') {
            continue;
        }
        fields.push((name.to_string(), value.to_string()));
    }

    fields
}

/// Return `text` with each of `overrides` applied to the `[Desktop Entry]`
/// group, replacing the key where it already exists and appending it to the end
/// of the group where it does not.
///
/// Everything else is preserved verbatim, including comments, other groups and
/// localised variants of keys that are not being overridden. A desktop entry
/// carries a great deal a package installer has no business discarding —
/// `MimeType`, `Categories`, `StartupWMClass` — and rebuilding the file from
/// the handful of fields this application understands would drop all of it.
pub fn rewrite(text: &str, overrides: &[(&str, String)]) -> String {
    let mut output: Vec<String> = Vec::new();
    let mut applied = vec![false; overrides.len()];
    let mut in_entry_group = false;
    // Where the group's last content line sits in `output`, so a key that was
    // not present can be appended to the group rather than to the file.
    let mut group_end: Option<usize> = None;

    for line in text.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with('[') {
            in_entry_group = trimmed == ENTRY_GROUP;
            output.push(line.to_string());
            if in_entry_group {
                group_end = Some(output.len());
            }
            continue;
        }

        if in_entry_group {
            if let Some((name, _)) = trimmed.split_once('=') {
                let name = name.trim();
                // Only the exact key is replaced, so `Icon` does not swallow
                // `Icon[de]` — a localised icon name is still wrong after the
                // rewrite, but silently deleting it would be worse.
                if let Some(index) = overrides.iter().position(|(key, _)| *key == name) {
                    if !applied[index] {
                        applied[index] = true;
                        output.push(format!("{}={}", name, overrides[index].1));
                        group_end = Some(output.len());
                        continue;
                    }
                }
            }
            if !trimmed.is_empty() {
                group_end = Some(output.len() + 1);
            }
        }

        output.push(line.to_string());
    }

    // Anything not already present is inserted at the end of the group.
    let mut insertion = group_end.unwrap_or(output.len());
    for (index, (key, value)) in overrides.iter().enumerate() {
        if applied[index] {
            continue;
        }
        output.insert(insertion, format!("{key}={value}"));
        insertion += 1;
    }

    let mut result = output.join("\n");
    result.push('\n');
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "[Desktop Entry]\n\
                          Type=Application\n\
                          Name=Real Name\n\
                          Name[de]=Deutscher Name\n\
                          Icon=my-icon\n\
                          Exec=inner-command %U\n\
                          Categories=Utility;\n\
                          \n\
                          [Desktop Action New]\n\
                          Name=Wrong\n\
                          Exec=other\n";

    #[test]
    fn reads_unlocalised_fields_from_the_entry_group_only() {
        assert_eq!(field(SAMPLE, "Name").as_deref(), Some("Real Name"));
        assert_eq!(field(SAMPLE, "Icon").as_deref(), Some("my-icon"));
        assert_eq!(field(SAMPLE, "Missing"), None);
        // The action group has a Name too, and it is not the application's.
        assert_eq!(
            field_in(SAMPLE, "[Desktop Action New]", "Name").as_deref(),
            Some("Wrong")
        );
    }

    #[test]
    fn reads_the_same_syntax_in_a_flatpak_metadata_file() {
        let metadata = "[Application]\n\
                        name=org.example.Hello\n\
                        runtime=org.freedesktop.Platform/x86_64/24.08\n\
                        \n\
                        [Context]\n\
                        sockets=wayland;\n";
        assert_eq!(
            field_in(metadata, "[Application]", "runtime").as_deref(),
            Some("org.freedesktop.Platform/x86_64/24.08")
        );
        assert_eq!(
            fields_in(metadata, "[Context]"),
            vec![("sockets".to_string(), "wayland;".to_string())]
        );
    }

    #[test]
    fn localised_keys_are_left_out_of_the_field_listing() {
        let listed = fields_in(SAMPLE, "[Desktop Entry]");
        assert!(listed.iter().any(|(key, _)| key == "Name"));
        assert!(!listed.iter().any(|(key, _)| key.contains('[')));
    }

    #[test]
    fn rewrites_existing_keys_in_place_and_leaves_other_groups_alone() {
        let result = rewrite(
            SAMPLE,
            &[
                ("Exec", "/home/u/.local/bin/app.AppImage".to_string()),
                ("Icon", "org.example.App".to_string()),
            ],
        );
        assert!(result.contains("Exec=/home/u/.local/bin/app.AppImage\n"));
        assert!(result.contains("Icon=org.example.App\n"));
        // The action group keeps its own Exec, and nothing else is lost.
        assert!(result.contains("[Desktop Action New]\nName=Wrong\nExec=other\n"));
        assert!(result.contains("Categories=Utility;"));
        assert!(result.contains("Name[de]=Deutscher Name"));
    }

    #[test]
    fn appends_keys_the_entry_did_not_have_inside_the_group() {
        let result = rewrite(SAMPLE, &[("X-AppImage-Source", "/tmp/a.AppImage".to_string())]);
        let group_end = result.find("[Desktop Action New]").unwrap();
        let key_at = result.find("X-AppImage-Source=/tmp/a.AppImage").unwrap();
        assert!(
            key_at < group_end,
            "a new key must land in [Desktop Entry], not after it"
        );
    }

    #[test]
    fn an_entry_with_no_group_header_still_gains_the_key() {
        let result = rewrite("", &[("Exec", "/bin/true".to_string())]);
        assert!(result.contains("Exec=/bin/true"));
    }
}
