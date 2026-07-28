// SPDX-License-Identifier: GPL-3.0

//! Keyboard shortcuts.
//!
//! The bindings live here rather than beside the menu definition so the menu
//! and the key handler cannot drift apart: [`crate::menu`] looks each action up
//! in this same map to print its shortcut, so a binding changed here changes
//! what the menu shows without any further edit.

use std::collections::HashMap;

use cosmic::iced::keyboard::Key;
use cosmic::widget::menu::key_bind::{KeyBind, Modifier};

use crate::menu::MenuAction;

/// The default bindings.
pub fn key_binds() -> HashMap<KeyBind, MenuAction> {
    let mut key_binds = HashMap::new();

    macro_rules! bind {
        ([$($modifier:ident),* $(,)?], $key:expr, $action:ident) => {{
            key_binds.insert(
                KeyBind {
                    modifiers: vec![$(Modifier::$modifier),*],
                    key: $key,
                },
                MenuAction::$action,
            );
        }};
    }

    bind!([Ctrl], Key::Character("o".into()), OpenPackage);
    bind!([Ctrl], Key::Character("w".into()), ClosePackage);
    bind!([Ctrl], Key::Character("r".into()), Reload);
    bind!([Ctrl], Key::Character("q".into()), Quit);
    bind!([Ctrl], Key::Character("1".into()), TabDetails);
    bind!([Ctrl], Key::Character("2".into()), TabDependencies);
    bind!([Ctrl], Key::Character("3".into()), TabFiles);
    bind!([Ctrl], Key::Character(",".into()), Settings);

    key_binds
}
