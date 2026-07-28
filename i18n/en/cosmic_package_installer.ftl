# Application
app-title = Package Installer
app-description = Install and inspect package files

# Menus
menu-file = File
menu-view = View
menu-help = Help
open-package = Open Package…
close-package = Close Package
quit = Quit
settings = Settings
about = About
reload = Reload

# Window
no-package-title = No package open
no-package-body = Open a .deb, .rpm, .flatpak or .appimage file to inspect and install it.
open-package-button = Open Package…
loading-package = Reading package…

# Tabs
tab-details = Details
tab-dependencies = Dependencies
tab-files = Files
toggle-sidebar = Show or hide the sidebar

# Installed state
state-not-installed = Not installed
state-installed = Version { $version } is installed
state-upgrade = Version { $version } is installed and can be upgraded
state-downgrade = Version { $version } is installed, which is newer than this file
state-unknown = Installed state unknown

# Actions
action-install = Install
action-reinstall = Reinstall
action-upgrade = Upgrade
action-downgrade = Downgrade
action-remove = Uninstall
cancel = Cancel
dismiss = Dismiss
retry = Try Again

# Progress
progress-installing = Installing
progress-reinstalling = Reinstalling
progress-upgrading = Upgrading
progress-downgrading = Downgrading
progress-removing = Uninstalling
operation-complete = Finished successfully
operation-failed = The operation did not complete

# Metadata
meta-section-package = Package
meta-section-other = Other
meta-package = Name
meta-version = Version
meta-format = Format
meta-architecture = Architecture
meta-maintainer = Maintainer
meta-section = Section
meta-license = License
meta-homepage = Homepage
meta-installed-size = Installed size
meta-file-size = File size
meta-path = File
meta-description = Description

# Dependency kinds
dep-pre-depends = Required before installation
dep-depends = Required
dep-recommends = Recommended
dep-suggests = Suggested
dep-conflicts = Conflicts with
dep-breaks = Breaks
dep-replaces = Replaces
dep-provides = Provides

# Dependency status
dep-status-installed = Installed ({ $version })
dep-status-available = Available ({ $version })
dep-status-provided-by = Provided by { $providers }
dep-status-missing = Not available
dep-status-unknown = Not checked
dep-alternatives = or
dependencies-none = This package declares no dependencies.
dependencies-resolving = Checking dependencies against your system…
dependencies-unsatisfiable = { $count ->
    [one] { $count } required dependency cannot be satisfied.
   *[other] { $count } required dependencies cannot be satisfied.
  }

# Install plan
plan-title = What will happen
plan-install = Install
plan-upgrade = Upgrade
plan-downgrade = Downgrade
plan-remove = Remove
plan-resolving = Working out what will be installed…
plan-no-changes = Nothing needs to change.
plan-additional = { $count ->
    [one] { $count } additional package will be installed.
   *[other] { $count } additional packages will be installed.
  }
plan-download = { $size } to download
plan-disk = { $size } of disk space
plan-blocked = This package cannot be installed as things stand:

# Files
files-none = This package installs no files.
files-hidden = The file list is turned off in Settings.
files-count = { $count ->
    [one] { $count } file
   *[other] { $count } files
  }
files-truncated = Showing the first { $shown } of { $total } entries.
files-link = links to { $target }

# Supported formats
supported-formats = Supported formats
format-deb = Debian package
format-rpm = RPM package
format-flatpak = Flatpak
format-appimage = AppImage
format-ready = Supported
format-missing = Needs { $tools }

# Settings
settings-appearance = Appearance
settings-theme = Theme
theme-dark = Dark
theme-light = Light
theme-system = Match desktop
settings-behaviour = Behaviour
settings-privilege-backend = Install using
privilege-auto = PackageKit, or system tools
privilege-packagekit = PackageKit only
privilege-native = System tools only
settings-show-recommends = Show recommended packages
settings-show-suggests = Show suggested packages
settings-show-file-list = Show the file list

# Confirmation
confirm-remove-title = Uninstall { $name }?
confirm-remove-body = This will remove the package from your system.

# About
about-repository = Repository
about-support = Report an issue

# Errors
error-unknown-format = { $file } is not a package file this application can open.
error-unsupported-format = { $format } files need { $tools }, which is not installed.
error-missing-tool = { $program } is not installed.
error-command-failed = { $program } reported a problem: { $detail }
error-timeout = { $program } took too long and was stopped.
error-parse = Could not make sense of the package: { $detail }
error-packagekit = PackageKit reported a problem: { $detail }
error-not-authorized = Authentication was cancelled or refused.
error-not-implemented = Support for { $format } files is not implemented yet.
