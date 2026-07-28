// SPDX-License-Identifier: GPL-3.0

//! A COSMIC Desktop package installer.
//!
//! Opens a local `.deb`, `.rpm`, `.flatpak` or `.appimage` file, shows what is
//! in it — icon, metadata, files, and dependencies with their status on this
//! system — and installs, upgrades or removes it.

mod app;
mod backend;
mod config;
mod constants;
mod debug;
mod i18n;
mod key_bind;
mod menu;

use std::path::PathBuf;

use cosmic::app::Settings;
use cosmic::cosmic_config::{self, CosmicConfigEntry};
use cosmic::iced::Limits;
use cosmic::Application;

use app::{App, Flags};
use config::{Config, CONFIG_VERSION};
use constants::{WINDOW_HEIGHT, WINDOW_MIN_HEIGHT, WINDOW_MIN_WIDTH, WINDOW_WIDTH};

fn main() -> cosmic::iced::Result {
    // The system's preferred languages, so the UI comes up localized.
    let requested_languages = i18n_embed::DesktopLanguageRequester::requested_languages();
    i18n::init(&requested_languages);

    let (config_handler, config) = load_config();

    // The file to open, if the application was launched by activating one.
    // Anything that is not an existing file is ignored rather than reported:
    // desktop environments pass a variety of arguments, and refusing to start
    // over one would be worse than opening to the empty state.
    let path = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .and_then(|path| path.canonicalize().ok())
        .filter(|path| path.is_file());

    let settings = Settings::default()
        .theme(config.app_theme.theme())
        .size_limits(
            Limits::NONE
                .min_width(WINDOW_MIN_WIDTH)
                .min_height(WINDOW_MIN_HEIGHT),
        )
        .size(cosmic::iced::Size::new(WINDOW_WIDTH, WINDOW_HEIGHT));

    cosmic::app::run::<App>(
        settings,
        Flags {
            config_handler,
            config,
            path,
        },
    )
}

/// Load the persisted configuration, falling back to defaults.
///
/// A config that fails to load is not fatal: the application starts with
/// defaults and reports the problem, rather than refusing to open the package
/// the user just double-clicked.
fn load_config() -> (Option<cosmic_config::Config>, Config) {
    match cosmic_config::Config::new(App::APP_ID, CONFIG_VERSION) {
        Ok(handler) => {
            let config = match Config::get_entry(&handler) {
                Ok(config) => config,
                Err((errors, config)) => {
                    eprintln!("errors loading configuration: {errors:?}");
                    config
                }
            };
            (Some(handler), config)
        }
        Err(error) => {
            eprintln!("failed to create the configuration handler: {error}");
            (None, Config::default())
        }
    }
}
