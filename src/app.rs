// SPDX-License-Identifier: GPL-3.0

//! The application: window, state machine, and views.
//!
//! The shape of the window follows COSMIC Store's application page — a large
//! icon, the name and summary, the action button, then the detail below — while
//! the menu bar follows COSMIC Files. The point is that neither is novel: this
//! is usually opened by double-clicking a downloaded file, which is not a
//! moment for learning a new interface.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use cosmic::app::{context_drawer, Core, Task};
use cosmic::cosmic_config::{self, CosmicConfigEntry};
use cosmic::iced::futures::channel::{mpsc, oneshot};
use cosmic::iced::keyboard::{Event as KeyEvent, Key, Modifiers};
use cosmic::iced::{event, Alignment, Event, Length, Subscription};
use cosmic::widget::menu::key_bind::KeyBind;
use cosmic::widget::nav_bar;
use cosmic::{theme, widget, Application, ApplicationExt, Element};

use crate::backend::{
    self, format_size, format_size_delta, Action, Availability, Backend, Dependency,
    DependencyKind, DependencyStatus, InstalledState, OperationPlan, PackageDetails, PackageFormat,
    PayloadEntry, Progress,
};
use crate::config::{AppTheme, Config, FlatpakScope, PrivilegeBackend, CONFIG_VERSION};
use crate::constants::{
    APP_ICON, APP_ID, FALLBACK_ICON, ICON_SIZE_HEADER, ICON_SIZE_ROW, ISSUES_URL,
    MAX_CONTENT_WIDTH, MAX_FILES_SHOWN, REPOSITORY_URL,
};
use crate::debug::UI;
use crate::key_bind::key_binds;
use crate::menu::{self, MenuAction};
use crate::{debug_log, fl};

/// Theme choices, in the order the settings dropdown lists them.
///
/// Paired with `App::theme_labels` by index. The labels are stored on the
/// application rather than built in the view because `widget::dropdown` borrows
/// its label slice for as long as the returned element lives.
const THEME_OPTIONS: [AppTheme; 3] = [AppTheme::System, AppTheme::Light, AppTheme::Dark];

/// Privilege-transport choices, paired with `App::privilege_labels` by index.
const PRIVILEGE_OPTIONS: [PrivilegeBackend; 3] = [
    PrivilegeBackend::Auto,
    PrivilegeBackend::PackageKit,
    PrivilegeBackend::Native,
];

/// Flatpak installation-scope choices, paired with `App::flatpak_scope_labels`.
const FLATPAK_SCOPE_OPTIONS: [FlatpakScope; 2] = [FlatpakScope::User, FlatpakScope::System];

/// Which section of the package is on screen.
///
/// Presented as a vertical navigation sidebar rather than a row of tabs, which
/// is what COSMIC Settings does and what makes the sections collapsible: below
/// roughly 650 pixels libcosmic condenses the sidebar into an overlay by
/// itself, and the header button toggles it at any width.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Tab {
    Details,
    Dependencies,
    Files,
}

impl Tab {
    /// Symbolic icon shown beside the section in the sidebar.
    ///
    /// All three resolve through the Cosmic icon theme's inheritance chain
    /// (Cosmic → Pop → Adwaita → hicolor).
    fn icon_name(self) -> &'static str {
        match self {
            Self::Details => "dialog-information-symbolic",
            Self::Dependencies => "application-x-addon-symbolic",
            Self::Files => "folder-symbolic",
        }
    }
}

/// Panels shown in the context drawer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContextPage {
    Settings,
    About,
    SupportedFormats,
}

/// Modal dialogs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DialogPage {
    /// Uninstalling cannot be undone, so it is the one action that asks first.
    ConfirmRemove,
}

/// Values needed to start up, gathered before the window exists.
pub struct Flags {
    pub config_handler: Option<cosmic_config::Config>,
    pub config: Config,
    /// The file named on the command line, if any.
    pub path: Option<PathBuf>,
}

#[derive(Clone, Debug)]
pub enum Message {
    /// Internal surface plumbing for the menu bar's popups.
    Surface(cosmic::surface::Action),
    /// A no-op, used when a background task is abandoned.
    None,

    OpenFileDialog,
    Open(PathBuf),
    ClosePackage,
    Reload,
    Quit,

    /// A package has been read, or could not be.
    ///
    /// The generation number is carried through every stage so results from a
    /// file the user has already navigated away from are discarded rather than
    /// overwriting the current one.
    Inspected(u64, Box<Result<Inspection, String>>),
    Resolved(u64, Box<Result<PackageDetails, String>>),
    Planned(u64, Box<Result<OperationPlan, String>>),

    SelectTab(Tab),
    ToggleNavBar,
    ToggleContextPage(ContextPage),
    DialogCancel,
    LaunchUrl(String),
    Key(Modifiers, Key),

    /// Begin a privileged operation.
    Perform(Action),
    OperationProgress(Progress),
    OperationFinished(Box<Result<(), String>>),
    DismissOutcome,

    ConfigTheme(AppTheme),
    ConfigPrivilegeBackend(PrivilegeBackend),
    ConfigFlatpakScope(FlatpakScope),
    ConfigShowRecommends(bool),
    ConfigShowSuggests(bool),
    ConfigShowFileList(bool),
    ConfigUpdated(Config),
}

/// The result of reading a package file.
#[derive(Clone, Debug)]
pub struct Inspection {
    pub details: PackageDetails,
    pub installed: InstalledState,
}

/// What the window is currently showing.
enum PackageState {
    Empty,
    Loading { path: PathBuf },
    Failed { path: PathBuf, message: String },
    Loaded(Box<Loaded>),
}

struct Loaded {
    details: PackageDetails,
    installed: InstalledState,
    /// True until dependency statuses have been filled in.
    resolving: bool,
    /// True until the package manager has answered "what would happen".
    planning: bool,
    plan: Option<OperationPlan>,
    /// Set when resolution or planning failed, so the tab can say why instead
    /// of showing an empty list that looks like "no dependencies".
    resolve_error: Option<String>,
}

/// A privileged operation in flight, or the outcome of the last one.
struct Operation {
    action: Action,
    /// `None` until the transport reports a percentage; PackageKit does,
    /// `apt-get` does not.
    fraction: Option<f32>,
    status: Option<String>,
    /// `None` while running, then the result.
    outcome: Option<Result<(), String>>,
}

pub struct App {
    core: Core,
    config: Config,
    config_handler: Option<cosmic_config::Config>,
    key_binds: HashMap<KeyBind, MenuAction>,
    menu_bar_id: widget::Id,

    /// Localized dropdown labels, held here because the widget borrows them.
    theme_labels: Vec<String>,
    privilege_labels: Vec<String>,
    flatpak_scope_labels: Vec<String>,

    package: PackageState,
    /// Incremented for every file opened, to discard stale background results.
    generation: u64,
    /// The section sidebar. Driven by libcosmic through [`Application::nav_model`],
    /// which is what makes it collapsible and gives it COSMIC's own styling.
    nav: nav_bar::Model,
    context_page: Option<ContextPage>,
    dialog: Option<DialogPage>,
    operation: Option<Operation>,
}

impl App {
    /// Run `work` on a dedicated thread, delivering the message it returns.
    ///
    /// A plain OS thread rather than the async executor: every backend call
    /// blocks for as long as `dpkg` or `apt` takes, which is exactly what an
    /// async executor must not be asked to do.
    fn background<F>(work: F) -> Task<Message>
    where
        F: FnOnce() -> Message + Send + 'static,
    {
        let (sender, receiver) = oneshot::channel();
        std::thread::spawn(move || {
            let _ = sender.send(work());
        });
        cosmic::task::future(async move { receiver.await.unwrap_or(Message::None) })
    }

    /// The backend for the currently open package, if it loaded.
    fn loaded(&self) -> Option<&Loaded> {
        match &self.package {
            PackageState::Loaded(loaded) => Some(loaded),
            _ => None,
        }
    }

    fn loaded_mut(&mut self) -> Option<&mut Loaded> {
        match &mut self.package {
            PackageState::Loaded(loaded) => Some(loaded),
            _ => None,
        }
    }

    /// Whether a package is open, which decides what the menu offers.
    fn has_package(&self) -> bool {
        !matches!(self.package, PackageState::Empty)
    }

    /// The path of whatever is open, for reloading.
    fn current_path(&self) -> Option<PathBuf> {
        match &self.package {
            PackageState::Empty => None,
            PackageState::Loading { path } | PackageState::Failed { path, .. } => {
                Some(path.clone())
            }
            PackageState::Loaded(loaded) => Some(PathBuf::from(&loaded.details.path)),
        }
    }

    /// Start reading `path`, replacing whatever is open.
    fn open(&mut self, path: PathBuf) -> Task<Message> {
        // Bumping the generation invalidates every result still in flight for
        // the previous file.
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        debug_log!(UI, "opening {} (generation {generation})", path.display());

        self.package = PackageState::Loading { path: path.clone() };
        self.operation = None;
        self.dialog = None;
        self.select_tab(Tab::Details);

        let include_payload = self.config.show_file_list;
        Task::batch([
            self.update_titles(Some(&path)),
            Self::background(move || {
                let result = (|| {
                    let backend = backend::backend_for_path(&path)?;
                    let details = backend.inspect(&path, include_payload)?;
                    let installed = backend.installed_state(&details)?;
                    Ok(Inspection { details, installed })
                })()
                .map_err(|error: backend::Error| {
                    debug_log!(UI, "inspection failed: {error}");
                    error.localized()
                });
                Message::Inspected(generation, Box::new(result))
            }),
        ])
    }

    /// Kick off dependency resolution and planning for a freshly read package.
    ///
    /// Both run after the window is already showing the package, because both
    /// consult the package manager and can take seconds on a cold cache. The
    /// metadata a user most often wants is visible the whole time.
    fn resolve_and_plan(&self, generation: u64) -> Task<Message> {
        let Some(loaded) = self.loaded() else {
            return Task::none();
        };
        let Ok(backend) = backend_for(&loaded.details) else {
            return Task::none();
        };

        let action = loaded.installed.primary_action();

        let resolve_backend = Arc::clone(&backend);
        let mut resolve_details = loaded.details.clone();
        let resolve = Self::background(move || {
            let result = resolve_backend
                .resolve_dependencies(&mut resolve_details)
                .map(|()| resolve_details)
                .map_err(|error| error.localized());
            Message::Resolved(generation, Box::new(result))
        });

        let plan_details = loaded.details.clone();
        let plan = Self::background(move || {
            let result = backend
                .plan(&plan_details, action)
                .map_err(|error| error.localized());
            Message::Planned(generation, Box::new(result))
        });

        Task::batch([resolve, plan])
    }

    /// Begin a privileged operation.
    fn perform(&mut self, action: Action) -> Task<Message> {
        let Some(loaded) = self.loaded() else {
            return Task::none();
        };
        let Ok(backend) = backend_for(&loaded.details) else {
            return Task::none();
        };
        let details = loaded.details.clone();

        debug_log!(UI, "starting {action:?} on {}", details.id);
        self.operation = Some(Operation {
            action,
            fraction: None,
            status: None,
            outcome: None,
        });

        // Progress arrives while the work is still running, so it cannot be a
        // single future — the operation feeds a stream of messages and ends by
        // sending its result down the same channel.
        let (sender, receiver) = mpsc::unbounded::<Message>();
        std::thread::spawn(move || {
            let progress_sender = sender.clone();
            let mut on_progress = move |progress: Progress| {
                let _ = progress_sender.unbounded_send(Message::OperationProgress(progress));
            };
            let result = backend
                .perform(&details, action, &mut on_progress)
                .map_err(|error| {
                    debug_log!(UI, "{action:?} failed: {error}");
                    error.localized()
                });
            let _ = sender.unbounded_send(Message::OperationFinished(Box::new(result)));
        });

        cosmic::task::stream(receiver)
    }

    /// After a successful operation the installed state has changed, so the
    /// package is re-read rather than left showing what used to be true.
    fn refresh_after_operation(&mut self) -> Task<Message> {
        match self.current_path() {
            Some(path) => self.open(path),
            None => Task::none(),
        }
    }

    fn select_tab(&mut self, tab: Tab) {
        // Resolved into a plain `Entity` before activating: holding the
        // iterator across the call would keep `self.nav` borrowed.
        let entity = self
            .nav
            .iter()
            .find(|entity| self.nav.data::<Tab>(*entity) == Some(&tab));
        if let Some(entity) = entity {
            self.nav.activate(entity);
        }
    }

    fn active_tab(&self) -> Tab {
        self.nav
            .active_data::<Tab>()
            .copied()
            .unwrap_or(Tab::Details)
    }

    /// Put the open file's name in the header and the window title.
    ///
    /// Passing `None` restores the plain application name, for when nothing is
    /// open.
    fn update_titles(&mut self, path: Option<&std::path::Path>) -> Task<Message> {
        let name = path
            .and_then(|path| path.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let title = if name.is_empty() {
            fl!("app-title")
        } else {
            format!("{name} — {}", fl!("app-title"))
        };
        self.set_header_title(title.clone());
        match self.core.main_window_id() {
            Some(id) => self.set_window_title(title, id),
            None => Task::none(),
        }
    }

    /// Persist one config change.
    fn save_config(&mut self) -> Task<Message> {
        apply_config(&self.config);
        if let Some(handler) = &self.config_handler {
            if let Err(errors) = self.config.write_entry(handler) {
                debug_log!(crate::debug::CONFIG, "failed to save config: {errors:?}");
                eprintln!("failed to save configuration: {errors:?}");
            }
        }
        cosmic::task::message(cosmic::action::app(Message::None))
    }
}

/// Whether `url` is an `http`/`https` address safe to hand to `xdg-open`.
///
/// Deliberately strict: the scheme must be exactly one of these, so a package's
/// "Homepage" cannot smuggle in `file://`, a custom scheme with a registered
/// handler, or a leading-dash string that a helper might read as an option.
fn is_web_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    for scheme in ["http://", "https://"] {
        if let Some(rest) = lower.strip_prefix(scheme) {
            // A scheme with something after it, and no control characters or
            // whitespace that have no business being in a URL.
            return !rest.is_empty()
                && !url.contains(|c: char| c.is_whitespace() || c.is_control());
        }
    }
    false
}

/// Push the settings that backends hold themselves out to those backends.
///
/// Two of them keep a process-wide preference rather than taking it as an
/// argument, because it belongs to the session rather than to the package being
/// operated on. Setting both in one place is what stops one of them being
/// forgotten on a path that changes the config.
fn apply_config(config: &Config) {
    backend::privileged::set_preference(config.privilege_backend);
    backend::flatpak::set_scope(config.flatpak_scope);
}

/// Look up the backend that handles an already-inspected package.
///
/// The format is known at this point, so this cannot legitimately fail — but
/// it returns a `Result` rather than unwrapping, because "the tool went missing
/// while the window was open" is a real if unlikely state and crashing on it
/// would lose whatever the user was looking at.
fn backend_for(details: &PackageDetails) -> backend::Result<Arc<dyn Backend>> {
    let backend = backend::backend_for(details.format);
    match backend.availability() {
        Availability::Ready => Ok(backend),
        Availability::Missing { tools } => Err(backend::Error::Unsupported {
            format: details.format,
            tools,
        }),
    }
}

impl Application for App {
    type Executor = cosmic::executor::Default;
    type Flags = Flags;
    type Message = Message;

    const APP_ID: &'static str = APP_ID;

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(core: Core, flags: Self::Flags) -> (Self, Task<Self::Message>) {
        apply_config(&flags.config);

        let mut nav = nav_bar::Model::default();
        nav.insert()
            .text(fl!("tab-details"))
            .icon(widget::icon::from_name(Tab::Details.icon_name()))
            .data(Tab::Details)
            .activate();
        nav.insert()
            .text(fl!("tab-dependencies"))
            .icon(widget::icon::from_name(Tab::Dependencies.icon_name()))
            .data(Tab::Dependencies);
        nav.insert()
            .text(fl!("tab-files"))
            .icon(widget::icon::from_name(Tab::Files.icon_name()))
            .data(Tab::Files);

        let mut app = App {
            core,
            config: flags.config,
            config_handler: flags.config_handler,
            key_binds: key_binds(),
            menu_bar_id: widget::Id::new("menu_bar"),
            theme_labels: vec![fl!("theme-system"), fl!("theme-light"), fl!("theme-dark")],
            privilege_labels: vec![
                fl!("privilege-auto"),
                fl!("privilege-packagekit"),
                fl!("privilege-native"),
            ],
            flatpak_scope_labels: vec![fl!("flatpak-scope-user"), fl!("flatpak-scope-system")],
            package: PackageState::Empty,
            generation: 0,
            nav,
            context_page: None,
            dialog: None,
            operation: None,
        };

        app.set_header_title(fl!("app-title"));

        let task = match flags.path {
            Some(path) => app.open(path),
            None => Task::none(),
        };

        (app, task)
    }

    fn header_start(&self) -> Vec<Element<'_, Message>> {
        let mut elements: Vec<Element<'_, Message>> = Vec::new();

        // libcosmic renders the sidebar but does not provide the control that
        // collapses it, so the application supplies one — as COSMIC Settings
        // does. Only shown when there is a sidebar to collapse.
        if self.loaded().is_some() {
            elements.push(
                widget::tooltip(
                    widget::button::icon(widget::icon::from_name("sidebar-show-symbolic"))
                        .on_press(Message::ToggleNavBar)
                        .padding(8),
                    widget::text(fl!("toggle-sidebar")),
                    widget::tooltip::Position::Bottom,
                )
                .into(),
            );
        }

        elements.push(menu::menu_bar(
            &self.core,
            &self.key_binds,
            self.menu_bar_id.clone(),
            self.has_package(),
        ));

        elements
    }

    /// The section sidebar, shown only once a package has been read.
    ///
    /// Returning `None` for every other state is what keeps the empty, loading
    /// and error screens free of a sidebar listing sections that have nothing
    /// in them.
    fn nav_model(&self) -> Option<&nav_bar::Model> {
        self.loaded().map(|_| &self.nav)
    }

    fn on_nav_select(&mut self, id: nav_bar::Id) -> Task<Message> {
        self.nav.activate(id);
        Task::none()
    }

    fn context_drawer(&self) -> Option<context_drawer::ContextDrawer<'_, Message>> {
        if !self.core.window.show_context {
            return None;
        }
        let page = self.context_page?;
        Some(match page {
            ContextPage::Settings => context_drawer::context_drawer(
                self.view_settings(),
                Message::ToggleContextPage(ContextPage::Settings),
            )
            .title(fl!("settings")),
            ContextPage::About => context_drawer::context_drawer(
                self.view_about(),
                Message::ToggleContextPage(ContextPage::About),
            )
            .title(fl!("about")),
            ContextPage::SupportedFormats => context_drawer::context_drawer(
                self.view_supported_formats(),
                Message::ToggleContextPage(ContextPage::SupportedFormats),
            )
            .title(fl!("supported-formats")),
        })
    }

    fn dialog(&self) -> Option<Element<'_, Message>> {
        let dialog = self.dialog.as_ref()?;
        let loaded = self.loaded()?;
        match dialog {
            DialogPage::ConfirmRemove => Some(
                widget::dialog()
                    .icon(widget::icon::from_name("dialog-warning-symbolic").size(64))
                    .title(fl!("confirm-remove-title", name = loaded.details.name.as_str()))
                    .body(fl!("confirm-remove-body"))
                    .primary_action(
                        widget::button::destructive(fl!("action-remove"))
                            .on_press(Message::Perform(Action::Remove)),
                    )
                    .secondary_action(
                        widget::button::standard(fl!("cancel")).on_press(Message::DialogCancel),
                    )
                    .into(),
            ),
        }
    }

    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        match message {
            Message::None => Task::none(),

            Message::Surface(action) => cosmic::task::message(cosmic::action::cosmic(
                cosmic::app::Action::Surface(action),
            )),

            Message::OpenFileDialog => cosmic::task::future(async move {
                use cosmic::dialog::file_chooser::open::Dialog;
                use cosmic::dialog::file_chooser::FileFilter;

                // A filter per format, plus one covering all of them, so the
                // common case needs no menu interaction.
                let mut all = FileFilter::new(&fl!("supported-formats"));
                let mut dialog = Dialog::new().title(fl!("open-package"));
                for format in PackageFormat::ALL {
                    let mut filter = FileFilter::new(&format.label());
                    for pattern in format.globs() {
                        all = all.glob(pattern);
                        filter = filter.glob(pattern);
                    }
                    dialog = dialog.filter(filter);
                }
                dialog = dialog.current_filter(all.clone()).filter(all);

                match dialog.open_file().await {
                    Ok(response) => match response.url().to_file_path() {
                        Ok(path) => Message::Open(path),
                        // A non-local URL cannot be handed to dpkg, and the
                        // portal only offers such a thing for remote mounts.
                        Err(()) => Message::None,
                    },
                    Err(error) => {
                        debug_log!(UI, "file chooser closed: {error:?}");
                        Message::None
                    }
                }
            }),

            Message::Open(path) => self.open(path),

            Message::ClosePackage => {
                self.package = PackageState::Empty;
                self.operation = None;
                // Invalidates anything still resolving for the closed file.
                self.generation = self.generation.wrapping_add(1);
                self.update_titles(None)
            }

            Message::Reload => match self.current_path() {
                Some(path) => self.open(path),
                None => Task::none(),
            },

            Message::Quit => cosmic::iced::exit(),

            Message::Inspected(generation, result) => {
                if generation != self.generation {
                    debug_log!(UI, "discarding stale inspection {generation}");
                    return Task::none();
                }
                match *result {
                    Ok(inspection) => {
                        debug_log!(
                            UI,
                            "loaded {} v{} ({:?})",
                            inspection.details.id,
                            inspection.details.version,
                            inspection.installed
                        );
                        self.package = PackageState::Loaded(Box::new(Loaded {
                            details: inspection.details,
                            installed: inspection.installed,
                            resolving: true,
                            planning: true,
                            plan: None,
                            resolve_error: None,
                        }));
                        self.resolve_and_plan(generation)
                    }
                    Err(message) => {
                        let path = self.current_path().unwrap_or_default();
                        self.package = PackageState::Failed { path, message };
                        Task::none()
                    }
                }
            }

            Message::Resolved(generation, result) => {
                if generation != self.generation {
                    return Task::none();
                }
                if let Some(loaded) = self.loaded_mut() {
                    loaded.resolving = false;
                    match *result {
                        Ok(details) => loaded.details = details,
                        Err(message) => loaded.resolve_error = Some(message),
                    }
                }
                Task::none()
            }

            Message::Planned(generation, result) => {
                if generation != self.generation {
                    return Task::none();
                }
                if let Some(loaded) = self.loaded_mut() {
                    loaded.planning = false;
                    match *result {
                        Ok(plan) => loaded.plan = Some(plan),
                        Err(message) => {
                            // A plan that could not be produced is reported in
                            // the same place a blocked plan would be, so the
                            // user learns something either way.
                            loaded.plan = Some(OperationPlan {
                                blocked: Some(message),
                                ..OperationPlan::default()
                            });
                        }
                    }
                }
                Task::none()
            }

            Message::SelectTab(tab) => {
                self.select_tab(tab);
                Task::none()
            }

            Message::ToggleNavBar => {
                self.core.nav_bar_toggle();
                Task::none()
            }

            Message::ToggleContextPage(page) => {
                if self.context_page == Some(page) && self.core.window.show_context {
                    self.core.window.show_context = false;
                } else {
                    self.context_page = Some(page);
                    self.core.window.show_context = true;
                }
                Task::none()
            }

            Message::DialogCancel => {
                self.dialog = None;
                Task::none()
            }

            Message::LaunchUrl(url) => {
                // The homepage is read out of the package, so it is not to be
                // trusted with `xdg-open`, which would hand a `file://` — or any
                // scheme with a registered handler — straight to the desktop.
                // Only real web links are opened; anything else is refused,
                // since a "Homepage" that is not a web address is not one.
                if is_web_url(&url) {
                    if let Err(error) = std::process::Command::new("xdg-open").arg(&url).spawn() {
                        eprintln!("failed to open {url}: {error}");
                    }
                } else {
                    debug_log!(UI, "refusing to open non-web URL {url:?}");
                }
                Task::none()
            }

            Message::Key(modifiers, key) => {
                for (key_bind, action) in &self.key_binds {
                    if key_bind.matches(modifiers, &key, None) {
                        return self.update(cosmic::widget::menu::Action::message(action));
                    }
                }
                Task::none()
            }

            Message::Perform(action) => {
                // Uninstalling is the only irreversible action here, so it is
                // the only one that asks first.
                if action == Action::Remove && self.dialog.is_none() {
                    self.dialog = Some(DialogPage::ConfirmRemove);
                    return Task::none();
                }
                self.dialog = None;
                self.perform(action)
            }

            Message::OperationProgress(progress) => {
                if let Some(operation) = &mut self.operation {
                    match progress {
                        Progress::Fraction(fraction) => {
                            operation.fraction = Some(fraction.clamp(0.0, 1.0))
                        }
                        Progress::Status(status) => operation.status = Some(status),
                    }
                }
                Task::none()
            }

            Message::OperationFinished(result) => {
                let succeeded = result.is_ok();
                if let Some(operation) = &mut self.operation {
                    operation.outcome = Some(*result);
                    operation.fraction = Some(1.0);
                }
                debug_log!(UI, "operation finished, success={succeeded}");
                if succeeded {
                    self.refresh_after_operation()
                } else {
                    Task::none()
                }
            }

            Message::DismissOutcome => {
                self.operation = None;
                Task::none()
            }

            Message::ConfigTheme(app_theme) => {
                self.config.app_theme = app_theme;
                let task = self.save_config();
                Task::batch([task, cosmic::command::set_theme(app_theme.theme())])
            }
            Message::ConfigPrivilegeBackend(value) => {
                self.config.privilege_backend = value;
                self.save_config()
            }
            Message::ConfigFlatpakScope(value) => {
                self.config.flatpak_scope = value;
                let save = self.save_config();
                // Whether the open package counts as installed, and what an
                // uninstall would remove, both depend on the scope — so the
                // package is re-read rather than left describing the other one.
                let reload = match self.current_path() {
                    Some(path) if self.has_package() => self.open(path),
                    _ => Task::none(),
                };
                Task::batch([save, reload])
            }
            Message::ConfigShowRecommends(value) => {
                self.config.show_recommends = value;
                self.save_config()
            }
            Message::ConfigShowSuggests(value) => {
                self.config.show_suggests = value;
                self.save_config()
            }
            Message::ConfigShowFileList(value) => {
                self.config.show_file_list = value;
                let save = self.save_config();
                // The payload is read during inspection, so turning the list
                // back on means reading the package again.
                let reload = if value && self.has_package() {
                    match self.current_path() {
                        Some(path) => self.open(path),
                        None => Task::none(),
                    }
                } else {
                    Task::none()
                };
                Task::batch([save, reload])
            }
            Message::ConfigUpdated(config) => {
                let theme_changed = config.app_theme != self.config.app_theme;
                self.config = config;
                apply_config(&self.config);
                if theme_changed {
                    cosmic::command::set_theme(self.config.app_theme.theme())
                } else {
                    Task::none()
                }
            }
        }
    }

    fn view(&self) -> Element<'_, Self::Message> {
        let spacing = theme::active().cosmic().spacing;

        let content: Element<_> = match &self.package {
            PackageState::Empty => self.view_empty(),
            PackageState::Loading { path } => self.view_loading(path),
            PackageState::Failed { path, message } => self.view_failed(path, message),
            PackageState::Loaded(loaded) => self.view_package(loaded),
        };

        widget::container(
            widget::container(content)
                .max_width(MAX_CONTENT_WIDTH)
                .width(Length::Fill),
        )
        .align_x(Alignment::Center)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(spacing.space_m)
        .into()
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        struct ConfigSubscription;

        Subscription::batch([
            event::listen_with(|event, status, _window| match event {
                Event::Keyboard(KeyEvent::KeyPressed {
                    key, modifiers, ..
                }) => match status {
                    // A shortcut must not fire while the user is typing into a
                    // widget that already handled the key.
                    event::Status::Ignored => Some(Message::Key(modifiers, key)),
                    event::Status::Captured => None,
                },
                _ => None,
            }),
            cosmic_config::config_subscription::<_, Config>(
                std::any::TypeId::of::<ConfigSubscription>(),
                Self::APP_ID.into(),
                CONFIG_VERSION,
            )
            .map(|update| Message::ConfigUpdated(update.config)),
        ])
    }
}

// ── Views ───────────────────────────────────────────────────────────────────

impl App {
    /// Nothing open: explain what to do, and what this system can handle.
    fn view_empty(&self) -> Element<'_, Message> {
        let spacing = theme::active().cosmic().spacing;

        widget::container(
            widget::column::with_children(vec![
                widget::icon::from_name(FALLBACK_ICON).size(64).into(),
                widget::text::title3(fl!("no-package-title")).into(),
                widget::text::body(fl!("no-package-body"))
                    .align_x(Alignment::Center)
                    .into(),
                widget::button::suggested(fl!("open-package-button"))
                    .on_press(Message::OpenFileDialog)
                    .into(),
            ])
            .spacing(spacing.space_s)
            .align_x(Alignment::Center),
        )
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    fn view_loading(&self, path: &std::path::Path) -> Element<'_, Message> {
        let spacing = theme::active().cosmic().spacing;
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();

        widget::container(
            widget::column::with_children(vec![
                widget::indeterminate_circular().size(32.0).into(),
                widget::text::body(fl!("loading-package")).into(),
                widget::text::caption(name).into(),
            ])
            .spacing(spacing.space_s)
            .align_x(Alignment::Center),
        )
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    fn view_failed(&self, path: &std::path::Path, message: &str) -> Element<'_, Message> {
        let spacing = theme::active().cosmic().spacing;
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();

        widget::container(
            widget::column::with_children(vec![
                widget::icon::from_name("dialog-error-symbolic")
                    .size(48)
                    .into(),
                widget::text::title4(name).into(),
                widget::text::body(message.to_string())
                    .align_x(Alignment::Center)
                    .into(),
                widget::row::with_children(vec![
                    widget::button::standard(fl!("retry"))
                        .on_press(Message::Reload)
                        .into(),
                    widget::button::suggested(fl!("open-package-button"))
                        .on_press(Message::OpenFileDialog)
                        .into(),
                ])
                .spacing(spacing.space_xs)
                .into(),
            ])
            .spacing(spacing.space_s)
            .align_x(Alignment::Center)
            .max_width(560.0),
        )
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    /// The main view: header, status, tabs, and the selected tab's content.
    fn view_package<'a>(&'a self, loaded: &'a Loaded) -> Element<'a, Message> {
        let spacing = theme::active().cosmic().spacing;

        let mut children: Vec<Element<'a, Message>> = vec![
            self.view_header(loaded),
            self.view_status_banner(loaded),
        ];

        if let Some(warning) = self.view_blocked_warning(loaded) {
            children.push(warning);
        }

        // No tab strip here: the sections are the sidebar, which libcosmic
        // renders and which `nav_model` supplies.
        let tab_content = match self.active_tab() {
            Tab::Details => self.view_details(loaded),
            Tab::Dependencies => self.view_dependencies(loaded),
            Tab::Files => self.view_files(loaded),
        };
        children.push(widget::scrollable(tab_content).height(Length::Fill).into());

        widget::column::with_children(children)
            .spacing(spacing.space_s)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    /// Icon, name, summary, and the action buttons — or progress, while an
    /// operation is running.
    fn view_header<'a>(&'a self, loaded: &'a Loaded) -> Element<'a, Message> {
        let spacing = theme::active().cosmic().spacing;
        let details = &loaded.details;

        let icon = match &details.icon {
            Some(handle) => widget::icon::icon(handle.clone()).size(ICON_SIZE_HEADER),
            None => widget::icon::from_name(FALLBACK_ICON).size(ICON_SIZE_HEADER).into(),
        };

        let mut identity: Vec<Element<'a, Message>> =
            vec![widget::text::title3(details.name.as_str()).into()];
        if let Some(summary) = &details.summary {
            identity.push(widget::text::body(summary.as_str()).into());
        }
        identity.push(
            widget::text::caption(format!(
                "{} · {}",
                details.version,
                details.format.label()
            ))
            .into(),
        );

        widget::row::with_children(vec![
            icon.into(),
            widget::column::with_children(identity)
                .spacing(spacing.space_xxxs)
                .width(Length::Fill)
                .into(),
            self.view_actions(loaded),
        ])
        .spacing(spacing.space_s)
        .align_y(Alignment::Center)
        .into()
    }

    /// The action buttons, or the progress of a running operation.
    fn view_actions<'a>(&'a self, loaded: &'a Loaded) -> Element<'a, Message> {
        let spacing = theme::active().cosmic().spacing;

        if let Some(operation) = &self.operation {
            return self.view_operation(operation);
        }

        let primary_action = loaded.installed.primary_action();

        // An install that the package manager says cannot go ahead is offered
        // anyway only when nothing definitive is known; when apt has actually
        // said no, the button is disabled rather than leading to a failure the
        // user was already warned about.
        let blocked = loaded
            .plan
            .as_ref()
            .is_some_and(|plan| plan.blocked.is_some())
            || !loaded.details.unsatisfiable().is_empty();

        let mut primary = widget::button::suggested(primary_action.label());
        if !blocked {
            primary = primary.on_press(Message::Perform(primary_action));
        }

        let mut buttons: Vec<Element<'a, Message>> = vec![primary.into()];
        if loaded.installed.is_installed() {
            buttons.push(
                widget::button::destructive(Action::Remove.label())
                    .on_press(Message::Perform(Action::Remove))
                    .into(),
            );
        }

        widget::column::with_children(buttons)
            .spacing(spacing.space_xxs)
            .align_x(Alignment::End)
            .into()
    }

    fn view_operation<'a>(&'a self, operation: &'a Operation) -> Element<'a, Message> {
        let spacing = theme::active().cosmic().spacing;

        let mut children: Vec<Element<'a, Message>> = Vec::new();
        match &operation.outcome {
            None => {
                children.push(widget::text::body(operation.action.progress_label()).into());
                // PackageKit reports a percentage; apt does not, and an
                // indeterminate bar is honest about not knowing rather than
                // inventing a figure.
                children.push(match operation.fraction {
                    Some(fraction) => widget::determinate_linear(fraction)
                        .width(Length::Fixed(220.0))
                        .into(),
                    None => widget::indeterminate_linear()
                        .width(Length::Fixed(220.0))
                        .into(),
                });
                if let Some(status) = &operation.status {
                    children.push(widget::text::caption(status.as_str()).into());
                }
            }
            Some(Ok(())) => {
                children.push(
                    widget::row::with_children(vec![
                        widget::icon::from_name("object-select-symbolic")
                            .size(ICON_SIZE_ROW)
                            .into(),
                        widget::text::body(fl!("operation-complete")).into(),
                    ])
                    .spacing(spacing.space_xxs)
                    .align_y(Alignment::Center)
                    .into(),
                );
                children.push(
                    widget::button::standard(fl!("dismiss"))
                        .on_press(Message::DismissOutcome)
                        .into(),
                );
            }
            Some(Err(message)) => {
                children.push(
                    widget::row::with_children(vec![
                        widget::icon::from_name("dialog-error-symbolic")
                            .size(ICON_SIZE_ROW)
                            .into(),
                        widget::text::body(fl!("operation-failed")).into(),
                    ])
                    .spacing(spacing.space_xxs)
                    .align_y(Alignment::Center)
                    .into(),
                );
                children.push(widget::text::caption(message.as_str()).into());
                children.push(
                    widget::button::standard(fl!("dismiss"))
                        .on_press(Message::DismissOutcome)
                        .into(),
                );
            }
        }

        widget::column::with_children(children)
            .spacing(spacing.space_xxs)
            .align_x(Alignment::End)
            .max_width(320.0)
            .into()
    }

    /// A one-line statement of whether this package is already on the system.
    fn view_status_banner<'a>(&'a self, loaded: &'a Loaded) -> Element<'a, Message> {
        let spacing = theme::active().cosmic().spacing;

        let (icon_name, text) = match &loaded.installed {
            InstalledState::NotInstalled => ("package-x-generic-symbolic", fl!("state-not-installed")),
            InstalledState::SameVersion { installed } => (
                "object-select-symbolic",
                fl!("state-installed", version = installed.as_str()),
            ),
            InstalledState::Older { installed } => (
                "go-up-symbolic",
                fl!("state-upgrade", version = installed.as_str()),
            ),
            InstalledState::Newer { installed } => (
                "go-down-symbolic",
                fl!("state-downgrade", version = installed.as_str()),
            ),
            InstalledState::Unknown => ("dialog-question-symbolic", fl!("state-unknown")),
        };

        widget::container(
            widget::row::with_children(vec![
                widget::icon::from_name(icon_name)
                    .size(ICON_SIZE_ROW)
                    .into(),
                widget::text::body(text).into(),
            ])
            .spacing(spacing.space_xxs)
            .align_y(Alignment::Center),
        )
        .class(theme::Container::Card)
        .padding([spacing.space_xxs, spacing.space_xs])
        .width(Length::Fill)
        .into()
    }

    /// Why the package cannot be installed, when it cannot.
    fn view_blocked_warning<'a>(&'a self, loaded: &'a Loaded) -> Option<Element<'a, Message>> {
        let spacing = theme::active().cosmic().spacing;

        let unsatisfiable = loaded.details.unsatisfiable();
        let blocked = loaded.plan.as_ref().and_then(|plan| plan.blocked.as_ref());

        if unsatisfiable.is_empty() && blocked.is_none() {
            return None;
        }

        let mut children: Vec<Element<'a, Message>> = Vec::new();
        if !unsatisfiable.is_empty() {
            children.push(
                widget::text::body(fl!(
                    "dependencies-unsatisfiable",
                    count = unsatisfiable.len()
                ))
                .into(),
            );
        }
        if let Some(reason) = blocked {
            children.push(widget::text::caption(reason.as_str()).into());
        }

        Some(
            widget::container(
                widget::row::with_children(vec![
                    widget::icon::from_name("dialog-warning-symbolic")
                        .size(ICON_SIZE_ROW)
                        .into(),
                    widget::column::with_children(children)
                        .spacing(spacing.space_xxxs)
                        .width(Length::Fill)
                        .into(),
                ])
                .spacing(spacing.space_xxs)
                .align_y(Alignment::Start),
            )
            .class(theme::Container::Card)
            .padding([spacing.space_xxs, spacing.space_xs])
            .width(Length::Fill)
            .into(),
        )
    }

    /// The metadata tab.
    fn view_details<'a>(&'a self, loaded: &'a Loaded) -> Element<'a, Message> {
        let spacing = theme::active().cosmic().spacing;
        let details = &loaded.details;

        let value = |text: String| -> Element<'a, Message> {
            widget::text::body(text)
                .align_x(Alignment::End)
                .width(Length::Shrink)
                .into()
        };

        let mut package = widget::settings::section().title(fl!("meta-section-package"));
        package = package.add(widget::settings::item(
            fl!("meta-package"),
            value(details.id.clone()),
        ));
        package = package.add(widget::settings::item(
            fl!("meta-version"),
            value(details.version.clone()),
        ));
        package = package.add(widget::settings::item(
            fl!("meta-format"),
            value(details.format.label()),
        ));
        if let Some(architecture) = &details.architecture {
            package = package.add(widget::settings::item(
                fl!("meta-architecture"),
                value(architecture.clone()),
            ));
        }
        if let Some(section) = &details.section {
            package = package.add(widget::settings::item(
                fl!("meta-section"),
                value(section.clone()),
            ));
        }
        if let Some(license) = &details.license {
            package = package.add(widget::settings::item(
                fl!("meta-license"),
                value(license.clone()),
            ));
        }
        if let Some(maintainer) = &details.maintainer {
            package = package.add(widget::settings::item(
                fl!("meta-maintainer"),
                value(maintainer.clone()),
            ));
        }
        if let Some(homepage) = &details.homepage {
            // Bound to a concrete `Element` first: `settings::item` is generic
            // over anything convertible, which leaves a bare `.into()` with
            // nothing to infer from.
            let link: Element<'a, Message> = widget::button::link(homepage.clone())
                .on_press(Message::LaunchUrl(homepage.clone()))
                .into();
            package = package.add(widget::settings::item(fl!("meta-homepage"), link));
        }
        if let Some(size) = details.installed_size {
            package = package.add(widget::settings::item(
                fl!("meta-installed-size"),
                value(format_size(size)),
            ));
        }
        if let Some(size) = details.file_size {
            package = package.add(widget::settings::item(
                fl!("meta-file-size"),
                value(format_size(size)),
            ));
        }
        package = package.add(widget::settings::item(
            fl!("meta-path"),
            value(details.path.clone()),
        ));

        let mut children: Vec<Element<'a, Message>> = vec![package.into()];

        if let Some(description) = &details.description {
            children.push(
                widget::settings::section()
                    .title(fl!("meta-description"))
                    .add(widget::text::body(description.as_str()).width(Length::Fill))
                    .into(),
            );
        }

        if !details.extra.is_empty() {
            let mut other = widget::settings::section().title(fl!("meta-section-other"));
            for (key, item) in &details.extra {
                other = other.add(widget::settings::item(key.clone(), value(item.clone())));
            }
            children.push(other.into());
        }

        widget::column::with_children(children)
            .spacing(spacing.space_s)
            .width(Length::Fill)
            .into()
    }

    /// The dependency tab: what the package asks for, and what will really
    /// happen if it is installed.
    fn view_dependencies<'a>(&'a self, loaded: &'a Loaded) -> Element<'a, Message> {
        let spacing = theme::active().cosmic().spacing;
        let mut children: Vec<Element<'a, Message>> = Vec::new();

        // The resolved plan first: it answers the question people actually
        // have, which is "what am I about to put on my machine".
        children.push(self.view_plan(loaded));

        if loaded.resolving {
            children.push(
                widget::row::with_children(vec![
                    widget::indeterminate_circular().size(16.0).into(),
                    widget::text::body(fl!("dependencies-resolving")).into(),
                ])
                .spacing(spacing.space_xxs)
                .align_y(Alignment::Center)
                .into(),
            );
        }

        if let Some(error) = &loaded.resolve_error {
            children.push(widget::text::body(error.as_str()).into());
        }

        if loaded.details.dependencies.is_empty() && !loaded.resolving {
            // "No dependencies" means something different in each format, and
            // the same sentence for all of them would be wrong for two: an
            // AppImage has none because it carries them, and a Flatpak
            // reference has none listed because the file simply does not say.
            children.push(
                widget::text::body(match loaded.details.format {
                    PackageFormat::AppImage => fl!("dependencies-bundled"),
                    PackageFormat::Flatpak => fl!("dependencies-flatpak"),
                    PackageFormat::Deb | PackageFormat::Rpm => fl!("dependencies-none"),
                })
                .into(),
            );
        }

        for kind in DependencyKind::DISPLAY_ORDER {
            // Recommends and Suggests are noisy enough to be worth a setting;
            // hiding them here rather than filtering during parsing means the
            // toggle takes effect without re-reading the package.
            match kind {
                DependencyKind::Recommends if !self.config.show_recommends => continue,
                DependencyKind::Suggests if !self.config.show_suggests => continue,
                _ => {}
            }

            let entries: Vec<&Dependency> = loaded.details.dependencies_of(*kind).collect();
            if entries.is_empty() {
                continue;
            }

            let mut section = widget::settings::section().title(kind.label());
            for dependency in entries {
                section = section.add(dependency_row(dependency, spacing.space_xxs));
            }
            children.push(section.into());
        }

        widget::column::with_children(children)
            .spacing(spacing.space_s)
            .width(Length::Fill)
            .into()
    }

    /// The resolved install set, with sizes.
    fn view_plan<'a>(&'a self, loaded: &'a Loaded) -> Element<'a, Message> {
        let spacing = theme::active().cosmic().spacing;

        if loaded.planning {
            return widget::row::with_children(vec![
                widget::indeterminate_circular().size(16.0).into(),
                widget::text::body(fl!("plan-resolving")).into(),
            ])
            .spacing(spacing.space_xxs)
            .align_y(Alignment::Center)
            .into();
        }

        let Some(plan) = &loaded.plan else {
            return widget::column::with_children(Vec::new()).into();
        };

        let mut section = widget::settings::section().title(fl!("plan-title"));

        if let Some(reason) = &plan.blocked {
            section = section.add(widget::text::body(fl!("plan-blocked")));
            section = section.add(widget::text::caption(reason.as_str()));
        }

        if plan.changes.is_empty() && plan.blocked.is_none() {
            section = section.add(widget::text::body(fl!("plan-no-changes")));
        }

        let additional = plan.additional_count(&loaded.details.id);
        if additional > 0 {
            section = section.add(widget::text::body(fl!("plan-additional", count = additional)));
        }

        // Size figures come from package metadata rather than apt's prose, so
        // they are shown whenever they could be computed at all.
        // Both strings read as a sentence with the figure inside them, in every
        // locale, so the size is an argument rather than a separate value
        // column — passing it as one is what leaves `{ $size }` on screen.
        if let Some(download) = plan.download_size.filter(|size| *size > 0) {
            section = section.add(widget::settings::item_row(vec![widget::text::body(fl!(
                "plan-download",
                size = format_size(download)
            ))
            .into()]));
        }
        if let Some(delta) = plan.disk_size_delta {
            section = section.add(widget::settings::item_row(vec![widget::text::body(fl!(
                "plan-disk",
                size = format_size_delta(delta)
            ))
            .into()]));
        }

        for change in &plan.changes {
            let detail = match (&change.current_version, &change.version) {
                (Some(current), Some(new)) => format!("{current} → {new}"),
                (Some(current), None) => current.clone(),
                (None, Some(new)) => new.clone(),
                (None, None) => String::new(),
            };
            section = section.add(widget::settings::item_row(vec![
                widget::icon::from_name(change.kind.icon_name())
                    .size(ICON_SIZE_ROW)
                    .into(),
                widget::column::with_children(vec![
                    widget::text::body(change.name.as_str()).into(),
                    widget::text::caption(format!("{} · {detail}", change.kind.label())).into(),
                ])
                .width(Length::Fill)
                .into(),
            ]));
        }

        section.into()
    }

    /// The file-list tab.
    fn view_files<'a>(&'a self, loaded: &'a Loaded) -> Element<'a, Message> {
        let spacing = theme::active().cosmic().spacing;

        if !self.config.show_file_list {
            return widget::text::body(fl!("files-hidden")).into();
        }

        // A format that cannot enumerate its payload has to say so. An empty
        // list here would read as "installs no files", which of a Flatpak
        // bundle is the opposite of the truth.
        if !loaded.details.payload_known {
            return widget::text::body(fl!("files-unavailable")).into();
        }

        let files: Vec<&PayloadEntry> = loaded
            .details
            .payload
            .iter()
            .filter(|entry| !entry.is_directory)
            .collect();

        if files.is_empty() {
            return widget::text::body(fl!("files-none")).into();
        }

        let mut children: Vec<Element<'a, Message>> = vec![widget::text::body(fl!(
            "files-count",
            count = files.len()
        ))
        .into()];

        // Rendering tens of thousands of rows costs far more than the last few
        // thousand paths are worth; the true total is stated instead.
        let shown = files.len().min(MAX_FILES_SHOWN);
        if shown < files.len() {
            children.push(
                widget::text::caption(fl!(
                    "files-truncated",
                    shown = shown,
                    total = files.len()
                ))
                .into(),
            );
        }

        let mut list = widget::settings::section();
        for entry in files.iter().take(shown) {
            let mut lines: Vec<Element<'a, Message>> =
                vec![widget::text::body(entry.path.as_str()).into()];
            if let Some(target) = &entry.link_target {
                lines.push(
                    widget::text::caption(fl!("files-link", target = target.as_str())).into(),
                );
            }

            let mut row: Vec<Element<'a, Message>> = vec![widget::column::with_children(lines)
                .width(Length::Fill)
                .into()];
            // A symlink's own size is zero and saying so is just noise, so the
            // size is shown only where it means something.
            if let Some(size) = entry.size.filter(|_| entry.link_target.is_none()) {
                row.push(widget::text::caption(format_size(size)).into());
            }

            list = list.add(widget::settings::item_row(row));
        }
        children.push(list.into());

        widget::column::with_children(children)
            .spacing(spacing.space_xs)
            .width(Length::Fill)
            .into()
    }

    fn view_settings(&self) -> Element<'_, Message> {
        let spacing = theme::active().cosmic().spacing;

        let theme_selected = THEME_OPTIONS
            .iter()
            .position(|option| *option == self.config.app_theme);
        let privilege_selected = PRIVILEGE_OPTIONS
            .iter()
            .position(|option| *option == self.config.privilege_backend);
        let flatpak_scope_selected = FLATPAK_SCOPE_OPTIONS
            .iter()
            .position(|option| *option == self.config.flatpak_scope);

        widget::column::with_children(vec![
            widget::settings::section()
                .title(fl!("settings-appearance"))
                .add(widget::settings::item(
                    fl!("settings-theme"),
                    widget::dropdown(&self.theme_labels, theme_selected, |index| {
                        Message::ConfigTheme(THEME_OPTIONS[index])
                    }),
                ))
                .into(),
            widget::settings::section()
                .title(fl!("settings-behaviour"))
                .add(widget::settings::item(
                    fl!("settings-privilege-backend"),
                    widget::dropdown(&self.privilege_labels, privilege_selected, |index| {
                        Message::ConfigPrivilegeBackend(PRIVILEGE_OPTIONS[index])
                    }),
                ))
                .add(widget::settings::item(
                    fl!("settings-flatpak-scope"),
                    widget::dropdown(
                        &self.flatpak_scope_labels,
                        flatpak_scope_selected,
                        |index| Message::ConfigFlatpakScope(FLATPAK_SCOPE_OPTIONS[index]),
                    ),
                ))
                .add(widget::settings::item(
                    fl!("settings-show-recommends"),
                    widget::toggler(self.config.show_recommends)
                        .on_toggle(Message::ConfigShowRecommends),
                ))
                .add(widget::settings::item(
                    fl!("settings-show-suggests"),
                    widget::toggler(self.config.show_suggests)
                        .on_toggle(Message::ConfigShowSuggests),
                ))
                .add(widget::settings::item(
                    fl!("settings-show-file-list"),
                    widget::toggler(self.config.show_file_list)
                        .on_toggle(Message::ConfigShowFileList),
                ))
                .into(),
        ])
        .spacing(spacing.space_m)
        .into()
    }

    /// Which formats this particular system can handle, and what is missing.
    fn view_supported_formats(&self) -> Element<'_, Message> {
        let spacing = theme::active().cosmic().spacing;
        let mut section = widget::settings::section();

        for (format, availability) in backend::all_availability() {
            let (icon_name, status) = match &availability {
                Availability::Ready => ("object-select-symbolic", fl!("format-ready")),
                Availability::Missing { tools } => (
                    "dialog-warning-symbolic",
                    fl!("format-missing", tools = tools.join(", ")),
                ),
            };
            section = section.add(widget::settings::item_row(vec![
                widget::icon::from_name(icon_name)
                    .size(ICON_SIZE_ROW)
                    .into(),
                widget::column::with_children(vec![
                    widget::text::body(format.label()).into(),
                    widget::text::caption(status).into(),
                ])
                .width(Length::Fill)
                .into(),
            ]));
        }

        widget::column::with_children(vec![section.into()])
            .spacing(spacing.space_m)
            .into()
    }

    fn view_about(&self) -> Element<'_, Message> {
        let spacing = theme::active().cosmic().spacing;

        widget::column::with_children(vec![
            // The application's own icon, which the packaging targets install
            // under this name, rather than the generic package icon used to
            // stand in for a package that ships none.
            widget::icon::from_name(APP_ICON).size(64).into(),
            widget::text::title3(fl!("app-title")).into(),
            widget::text::body(env!("CARGO_PKG_VERSION")).into(),
            widget::text::caption(fl!("app-description")).into(),
            widget::button::link(fl!("about-repository"))
                .on_press(Message::LaunchUrl(REPOSITORY_URL.to_string()))
                .into(),
            widget::button::link(fl!("about-support"))
                .on_press(Message::LaunchUrl(ISSUES_URL.to_string()))
                .into(),
        ])
        .spacing(spacing.space_xs)
        .align_x(Alignment::Center)
        .width(Length::Fill)
        .into()
    }
}

/// One row of the dependency list.
fn dependency_row<'a>(dependency: &'a Dependency, spacing: u16) -> Element<'a, Message> {
    let status = dependency.status();

    // For `Conflicts` and `Breaks`, an installed package is the bad outcome and
    // a missing one is fine, so the icon is chosen from the inverted reading
    // rather than from the status alone.
    let icon_name = if dependency.kind.is_negative() {
        match status {
            DependencyStatus::Installed { .. } => "dialog-warning-symbolic",
            _ => "object-select-symbolic",
        }
    } else {
        status.icon_name()
    };

    let alternatives = dependency
        .alternatives
        .iter()
        .map(|alternative| alternative.display())
        .collect::<Vec<_>>()
        .join(&format!(" {} ", fl!("dep-alternatives")));

    widget::settings::item_row(vec![
        widget::icon::from_name(icon_name)
            .size(ICON_SIZE_ROW)
            .into(),
        widget::column::with_children(vec![
            widget::text::body(alternatives).into(),
            widget::text::caption(status.label()).into(),
        ])
        .spacing(spacing / 2)
        .width(Length::Fill)
        .into(),
    ])
    .into()
}

#[cfg(test)]
mod tests {
    use super::is_web_url;

    #[test]
    fn only_web_urls_are_opened() {
        assert!(is_web_url("https://example.org/notes"));
        assert!(is_web_url("http://example.org"));
        assert!(is_web_url("HTTPS://Example.ORG/x"));

        // The shapes a hostile "Homepage" might take.
        assert!(!is_web_url("file:///etc/passwd"));
        assert!(!is_web_url("smb://server/share"));
        assert!(!is_web_url("javascript:alert(1)"));
        assert!(!is_web_url("-forcedelete"));
        assert!(!is_web_url("https://"));
        assert!(!is_web_url("https://exa mple.org"));
        assert!(!is_web_url("https://example.org/\nrm"));
        assert!(!is_web_url(""));
    }
}
