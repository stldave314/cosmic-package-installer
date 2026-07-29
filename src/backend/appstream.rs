// SPDX-License-Identifier: GPL-3.0

//! Reading AppStream metadata.
//!
//! Flatpak bundles and better-behaved AppImages both carry an AppStream
//! component describing themselves, and it is a far better source than anything
//! else either format offers: a Flatpak's `metadata` file knows the application
//! ID and its runtime but not its name, and an AppImage's desktop entry knows
//! its name but not its licence, developer or release version.
//!
//! Only the fields this application displays are read, and only in their
//! unlocalised form — matching `xml:lang` against the user's locale properly is
//! the AppStream library's job, and doing it badly would show a German summary
//! to a French user.

use crate::debug::FLATPAK;
use crate::debug_log;

/// The parts of an AppStream component worth showing. Every field is optional
/// because every field is optional in the format.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Component {
    pub id: Option<String>,
    pub name: Option<String>,
    pub summary: Option<String>,
    pub description: Option<String>,
    pub license: Option<String>,
    pub developer: Option<String>,
    pub homepage: Option<String>,
    /// Version of the most recent `<release>`, which is the closest thing
    /// AppStream has to "the version of this file".
    pub version: Option<String>,
}

impl Component {
    fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// Parse an AppStream document, which may be a bare `<component>` or a
/// `<components>` collection.
///
/// When it is a collection, `preferred_id` picks the component out of it;
/// Flatpak writes a collection containing exactly one component, but the format
/// permits more and choosing the wrong one would describe the wrong program.
pub fn parse(xml: &str, preferred_id: Option<&str>) -> Option<Component> {
    let document = match roxmltree::Document::parse(xml) {
        Ok(document) => document,
        Err(error) => {
            debug_log!(FLATPAK, "AppStream data did not parse: {error}");
            return None;
        }
    };

    let root = document.root_element();
    let components: Vec<roxmltree::Node<'_, '_>> = if root.has_tag_name("component") {
        vec![root]
    } else {
        root.children()
            .filter(|node| node.has_tag_name("component"))
            .collect()
    };

    // AppStream IDs are written both bare and with a `.desktop` suffix
    // depending on the specification revision, so a match ignores it.
    let matches = |node: &roxmltree::Node<'_, '_>| -> bool {
        let Some(wanted) = preferred_id else {
            return false;
        };
        text_of(node, "id").is_some_and(|id| {
            let id = id.strip_suffix(".desktop").unwrap_or(&id);
            id == wanted
        })
    };

    let chosen = components
        .iter()
        .find(|node| matches(node))
        .or_else(|| components.first())?;

    let component = Component {
        id: text_of(chosen, "id")
            .map(|id| id.strip_suffix(".desktop").unwrap_or(&id).to_string()),
        name: text_of(chosen, "name"),
        summary: text_of(chosen, "summary"),
        description: chosen
            .children()
            .find(|node| node.has_tag_name("description") && !is_localised(node))
            .map(|node| flatten_description(&node))
            .filter(|text| !text.is_empty()),
        license: text_of(chosen, "project_license"),
        developer: developer_of(chosen),
        homepage: homepage_of(chosen),
        version: latest_release_version(chosen),
    };

    (!component.is_empty()).then_some(component)
}

/// Whether a node is a translation of its sibling rather than the original.
fn is_localised(node: &roxmltree::Node<'_, '_>) -> bool {
    node.attribute(("http://www.w3.org/XML/1998/namespace", "lang"))
        .is_some()
        || node.attribute("xml:lang").is_some()
}

/// The text of the first unlocalised child element named `tag`.
fn text_of(parent: &roxmltree::Node<'_, '_>, tag: &str) -> Option<String> {
    parent
        .children()
        .find(|node| node.has_tag_name(tag) && !is_localised(node))
        .and_then(|node| node.text())
        .map(collapse)
        .filter(|text| !text.is_empty())
}

/// The developer's name, from either spelling the specification has used:
/// `<developer_name>` before AppStream 1.0, `<developer><name>` after it.
fn developer_of(component: &roxmltree::Node<'_, '_>) -> Option<String> {
    if let Some(name) = text_of(component, "developer_name") {
        return Some(name);
    }
    let developer = component
        .children()
        .find(|node| node.has_tag_name("developer"))?;
    text_of(&developer, "name")
}

fn homepage_of(component: &roxmltree::Node<'_, '_>) -> Option<String> {
    component
        .children()
        .filter(|node| node.has_tag_name("url"))
        .find(|node| node.attribute("type") == Some("homepage"))
        .and_then(|node| node.text())
        .map(collapse)
        .filter(|text| !text.is_empty())
}

/// The version of the newest release.
///
/// Releases are conventionally listed newest first, but that is a convention
/// rather than a rule, so the one with the highest timestamp wins and the first
/// listed is used only when no release says when it happened.
fn latest_release_version(component: &roxmltree::Node<'_, '_>) -> Option<String> {
    let releases = component
        .children()
        .find(|node| node.has_tag_name("releases"))?;

    let entries: Vec<roxmltree::Node<'_, '_>> = releases
        .children()
        .filter(|node| node.has_tag_name("release"))
        .collect();

    let newest = entries
        .iter()
        .filter_map(|node| {
            node.attribute("timestamp")
                .and_then(|value| value.trim().parse::<i64>().ok())
                .map(|timestamp| (timestamp, node))
        })
        .max_by_key(|(timestamp, _)| *timestamp)
        .map(|(_, node)| *node)
        .or_else(|| entries.first().copied())?;

    newest
        .attribute("version")
        .map(collapse)
        .filter(|version| !version.is_empty())
}

/// Render a `<description>` as plain text.
///
/// Paragraphs become blank-line-separated blocks and list items become bulleted
/// lines, which is as much structure as the detail view can show.
fn flatten_description(description: &roxmltree::Node<'_, '_>) -> String {
    let mut blocks: Vec<String> = Vec::new();

    for node in description.children() {
        if is_localised(&node) {
            continue;
        }
        if node.has_tag_name("p") {
            let text = collapse(&all_text(&node));
            if !text.is_empty() {
                blocks.push(text);
            }
        } else if node.has_tag_name("ul") || node.has_tag_name("ol") {
            let items: Vec<String> = node
                .children()
                .filter(|item| item.has_tag_name("li") && !is_localised(item))
                .map(|item| format!("• {}", collapse(&all_text(&item))))
                .filter(|item| item.len() > 2)
                .collect();
            if !items.is_empty() {
                blocks.push(items.join("\n"));
            }
        }
    }

    blocks.join("\n\n")
}

/// All descendant text of a node, so inline markup inside a paragraph does not
/// truncate it.
///
/// Only genuine text nodes are collected. `descendants()` yields the element
/// itself as well as its children, and `text()` on an element returns its first
/// text child — so taking the text of everything would count the opening run of
/// each paragraph twice.
fn all_text(node: &roxmltree::Node<'_, '_>) -> String {
    node.descendants()
        .filter(roxmltree::Node::is_text)
        .filter_map(|child| child.text())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Collapse the indentation XML brings with it into single spaces.
fn collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    const COLLECTION: &str = r#"<?xml version="1.0"?>
        <components version="0.8" origin="flatpak">
          <component type="desktop">
            <id>org.example.Hello.desktop</id>
            <name>Hello Example</name>
            <name xml:lang="de">Hallo Beispiel</name>
            <summary>A tiny test application</summary>
            <project_license>MIT</project_license>
            <developer_name>Example Developer</developer_name>
            <url type="bugtracker">https://example.org/bugs</url>
            <url type="homepage">https://example.org/hello</url>
            <description>
              <p>First
                 paragraph.</p>
              <ul><li>one</li><li>two</li></ul>
              <p xml:lang="de">Nicht dieser.</p>
            </description>
            <releases>
              <release version="1.0.0" timestamp="100"/>
              <release version="1.2.3" timestamp="900"/>
            </releases>
          </component>
        </components>"#;

    #[test]
    fn reads_a_flatpak_collection() {
        let component = parse(COLLECTION, Some("org.example.Hello")).unwrap();
        assert_eq!(component.id.as_deref(), Some("org.example.Hello"));
        assert_eq!(component.name.as_deref(), Some("Hello Example"));
        assert_eq!(component.summary.as_deref(), Some("A tiny test application"));
        assert_eq!(component.license.as_deref(), Some("MIT"));
        assert_eq!(component.developer.as_deref(), Some("Example Developer"));
        assert_eq!(component.homepage.as_deref(), Some("https://example.org/hello"));
        // The newest release wins, not the first listed.
        assert_eq!(component.version.as_deref(), Some("1.2.3"));
        assert_eq!(
            component.description.as_deref(),
            Some("First paragraph.\n\n• one\n• two")
        );
    }

    #[test]
    fn reads_a_bare_component_and_the_post_1_0_developer_spelling() {
        let xml = r#"<component type="desktop-application">
              <id>org.example.Solo</id>
              <name>Solo</name>
              <developer id="org.example"><name>New Style</name></developer>
            </component>"#;
        let component = parse(xml, None).unwrap();
        assert_eq!(component.name.as_deref(), Some("Solo"));
        assert_eq!(component.developer.as_deref(), Some("New Style"));
        assert_eq!(component.version, None);
    }

    #[test]
    fn picks_the_component_matching_the_wanted_id() {
        let xml = r#"<components>
              <component><id>org.example.Other</id><name>Other</name></component>
              <component><id>org.example.Wanted</id><name>Wanted</name></component>
            </components>"#;
        let component = parse(xml, Some("org.example.Wanted")).unwrap();
        assert_eq!(component.name.as_deref(), Some("Wanted"));
    }

    #[test]
    fn malformed_xml_is_not_fatal() {
        assert!(parse("<component><id>unclosed", None).is_none());
        assert!(parse("", None).is_none());
    }
}
