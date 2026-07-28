# Application
app-title = Paketinstallation
app-description = Paketdateien installieren und untersuchen

# Menus
menu-file = Datei
menu-view = Ansicht
menu-help = Hilfe
open-package = Paket öffnen…
close-package = Paket schließen
quit = Beenden
settings = Einstellungen
about = Über
reload = Neu laden

# Window
no-package-title = Kein Paket geöffnet
no-package-body = Öffnen Sie eine .deb-, .rpm-, .flatpak- oder .appimage-Datei, um sie zu untersuchen und zu installieren.
open-package-button = Paket öffnen…
loading-package = Paket wird gelesen…

# Tabs
tab-details = Details
tab-dependencies = Abhängigkeiten
tab-files = Dateien
toggle-sidebar = Seitenleiste ein- oder ausblenden

# Installed state
state-not-installed = Nicht installiert
state-installed = Version { $version } ist installiert
state-upgrade = Version { $version } ist installiert und kann aktualisiert werden
state-downgrade = Version { $version } ist installiert und neuer als diese Datei
state-unknown = Installationsstatus unbekannt

# Actions
action-install = Installieren
action-reinstall = Neu installieren
action-upgrade = Aktualisieren
action-downgrade = Herabstufen
action-remove = Deinstallieren
cancel = Abbrechen
dismiss = Schließen
retry = Erneut versuchen

# Progress
progress-installing = Wird installiert
progress-reinstalling = Wird neu installiert
progress-upgrading = Wird aktualisiert
progress-downgrading = Wird herabgestuft
progress-removing = Wird deinstalliert
operation-complete = Erfolgreich abgeschlossen
operation-failed = Der Vorgang wurde nicht abgeschlossen

# Metadata
meta-section-package = Paket
meta-section-other = Weitere
meta-package = Name
meta-version = Version
meta-format = Format
meta-architecture = Architektur
meta-maintainer = Betreuer
meta-section = Bereich
meta-license = Lizenz
meta-homepage = Website
meta-installed-size = Installierte Größe
meta-file-size = Dateigröße
meta-path = Datei
meta-description = Beschreibung

# Dependency kinds
dep-pre-depends = Vor der Installation erforderlich
dep-depends = Erforderlich
dep-recommends = Empfohlen
dep-suggests = Vorgeschlagen
dep-conflicts = Steht in Konflikt mit
dep-breaks = Beschädigt
dep-replaces = Ersetzt
dep-provides = Stellt bereit

# Dependency status
dep-status-installed = Installiert ({ $version })
dep-status-available = Verfügbar ({ $version })
dep-status-provided-by = Bereitgestellt von { $providers }
dep-status-missing = Nicht verfügbar
dep-status-unknown = Nicht geprüft
dep-alternatives = oder
dependencies-none = Dieses Paket deklariert keine Abhängigkeiten.
dependencies-resolving = Abhängigkeiten werden mit Ihrem System abgeglichen…
dependencies-unsatisfiable = { $count ->
    [one] { $count } erforderliche Abhängigkeit kann nicht erfüllt werden.
   *[other] { $count } erforderliche Abhängigkeiten können nicht erfüllt werden.
  }

# Install plan
plan-title = Was geschehen wird
plan-install = Installieren
plan-upgrade = Aktualisieren
plan-downgrade = Herabstufen
plan-remove = Entfernen
plan-resolving = Es wird ermittelt, was installiert wird…
plan-no-changes = Es muss nichts geändert werden.
plan-additional = { $count ->
    [one] { $count } zusätzliches Paket wird installiert.
   *[other] { $count } zusätzliche Pakete werden installiert.
  }
plan-download = { $size } herunterzuladen
plan-disk = { $size } Speicherplatz
plan-blocked = Dieses Paket kann derzeit nicht installiert werden:

# Files
files-none = Dieses Paket installiert keine Dateien.
files-hidden = Die Dateiliste ist in den Einstellungen deaktiviert.
files-count = { $count ->
    [one] { $count } Datei
   *[other] { $count } Dateien
  }
files-truncated = Die ersten { $shown } von { $total } Einträgen werden angezeigt.
files-link = verweist auf { $target }

# Supported formats
supported-formats = Unterstützte Formate
format-deb = Debian-Paket
format-rpm = RPM-Paket
format-flatpak = Flatpak
format-appimage = AppImage
format-ready = Unterstützt
format-missing = Benötigt { $tools }

# Settings
settings-appearance = Erscheinungsbild
settings-theme = Design
theme-dark = Dunkel
theme-light = Hell
theme-system = An Desktop anpassen
settings-behaviour = Verhalten
settings-privilege-backend = Installieren mit
privilege-auto = PackageKit oder Systemwerkzeuge
privilege-packagekit = Nur PackageKit
privilege-native = Nur Systemwerkzeuge
settings-show-recommends = Empfohlene Pakete anzeigen
settings-show-suggests = Vorgeschlagene Pakete anzeigen
settings-show-file-list = Dateiliste anzeigen

# Confirmation
confirm-remove-title = { $name } deinstallieren?
confirm-remove-body = Dadurch wird das Paket von Ihrem System entfernt.

# About
about-repository = Repository
about-support = Problem melden

# Errors
error-unknown-format = { $file } ist keine Paketdatei, die diese Anwendung öffnen kann.
error-unsupported-format = { $format }-Dateien benötigen { $tools }, was nicht installiert ist.
error-missing-tool = { $program } ist nicht installiert.
error-command-failed = { $program } hat ein Problem gemeldet: { $detail }
error-timeout = { $program } hat zu lange gebraucht und wurde beendet.
error-parse = Das Paket konnte nicht ausgewertet werden: { $detail }
error-packagekit = PackageKit hat ein Problem gemeldet: { $detail }
error-not-authorized = Die Authentifizierung wurde abgebrochen oder verweigert.
error-not-implemented = Unterstützung für { $format }-Dateien ist noch nicht implementiert.
