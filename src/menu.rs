// SPDX-License-Identifier: GPL-3.0

//! The window's menu bar.
//!
//! Built with `responsive_menu_bar` so the menus collapse into a single
//! hamburger button when the window is too narrow for them — the same
//! behaviour as COSMIC Files, which is what a user opening a package file will
//! have seen most recently.

use std::collections::HashMap;

use cosmic::app::Core;
use cosmic::widget::menu::{key_bind::KeyBind, Item, ItemHeight, ItemWidth};
use cosmic::widget::responsive_menu_bar;
use cosmic::Element;

use crate::app::{ContextPage, Message, Tab};
use crate::fl;

/// Everything reachable from the menu bar or a keyboard shortcut.
///
/// Separate from [`Message`] because the menu widget needs `Copy` actions it
/// can compare, in order to find and display each item's shortcut.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MenuAction {
    OpenPackage,
    ClosePackage,
    Reload,
    Quit,
    TabDetails,
    TabDependencies,
    TabFiles,
    SupportedFormats,
    Settings,
    About,
}

impl cosmic::widget::menu::Action for MenuAction {
    type Message = Message;

    fn message(&self) -> Self::Message {
        match self {
            Self::OpenPackage => Message::OpenFileDialog,
            Self::ClosePackage => Message::ClosePackage,
            Self::Reload => Message::Reload,
            Self::Quit => Message::Quit,
            Self::TabDetails => Message::SelectTab(Tab::Details),
            Self::TabDependencies => Message::SelectTab(Tab::Dependencies),
            Self::TabFiles => Message::SelectTab(Tab::Files),
            Self::SupportedFormats => Message::ToggleContextPage(ContextPage::SupportedFormats),
            Self::Settings => Message::ToggleContextPage(ContextPage::Settings),
            Self::About => Message::ToggleContextPage(ContextPage::About),
        }
    }
}

/// Build the menu bar.
///
/// `has_package` greys out the entries that make no sense with nothing open,
/// rather than hiding them: a menu whose contents change shape as you use the
/// application is harder to learn than one where some items are simply
/// unavailable.
pub fn menu_bar<'a>(
    core: &Core,
    key_binds: &HashMap<KeyBind, MenuAction>,
    id: cosmic::widget::Id,
    has_package: bool,
) -> Element<'a, Message> {
    // `Item::Button` is enabled, `Item::ButtonDisabled` is not; picking between
    // them is how an item is greyed out.
    let package_item = |label: String, action: MenuAction| {
        if has_package {
            Item::Button(label, None, action)
        } else {
            Item::ButtonDisabled(label, None, action)
        }
    };

    responsive_menu_bar()
        .item_height(ItemHeight::Dynamic(40))
        .item_width(ItemWidth::Uniform(260))
        .spacing(4.0)
        .into_element(
            core,
            key_binds,
            id,
            Message::Surface,
            vec![
                (
                    fl!("menu-file"),
                    vec![
                        Item::Button(fl!("open-package"), None, MenuAction::OpenPackage),
                        package_item(fl!("reload"), MenuAction::Reload),
                        package_item(fl!("close-package"), MenuAction::ClosePackage),
                        Item::Divider,
                        Item::Button(fl!("quit"), None, MenuAction::Quit),
                    ],
                ),
                (
                    fl!("menu-view"),
                    vec![
                        package_item(fl!("tab-details"), MenuAction::TabDetails),
                        package_item(fl!("tab-dependencies"), MenuAction::TabDependencies),
                        package_item(fl!("tab-files"), MenuAction::TabFiles),
                        Item::Divider,
                        Item::Button(
                            fl!("supported-formats"),
                            None,
                            MenuAction::SupportedFormats,
                        ),
                        Item::Button(fl!("settings"), None, MenuAction::Settings),
                    ],
                ),
                (
                    fl!("menu-help"),
                    vec![Item::Button(fl!("about"), None, MenuAction::About)],
                ),
            ],
        )
}
