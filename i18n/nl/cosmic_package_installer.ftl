# Application
app-title = Pakketinstallatie
app-description = Pakketbestanden installeren en bekijken

# Menus
menu-file = Bestand
menu-view = Beeld
menu-help = Help
open-package = Pakket openen…
close-package = Pakket sluiten
quit = Afsluiten
settings = Instellingen
about = Over
reload = Herladen

# Window
no-package-title = Geen pakket geopend
no-package-body = Open een .deb-, .rpm-, .flatpak- of .appimage-bestand om het te bekijken en te installeren.
open-package-button = Pakket openen…
loading-package = Pakket wordt gelezen…

# Tabs
tab-details = Details
tab-dependencies = Afhankelijkheden
tab-files = Bestanden
toggle-sidebar = Zijbalk tonen of verbergen

# Installed state
state-not-installed = Niet geïnstalleerd
state-installed = Versie { $version } is geïnstalleerd
state-upgrade = Versie { $version } is geïnstalleerd en kan worden bijgewerkt
state-downgrade = Versie { $version } is geïnstalleerd en nieuwer dan dit bestand
state-unknown = Installatiestatus onbekend

# Actions
action-install = Installeren
action-reinstall = Opnieuw installeren
action-upgrade = Bijwerken
action-downgrade = Terugzetten
action-remove = Verwijderen
cancel = Annuleren
dismiss = Sluiten
retry = Opnieuw proberen

# Progress
progress-installing = Bezig met installeren
progress-reinstalling = Bezig met opnieuw installeren
progress-upgrading = Bezig met bijwerken
progress-downgrading = Bezig met terugzetten
progress-removing = Bezig met verwijderen
progress-copying = Bezig met kopiëren van de AppImage…
progress-integrating = Bezig met toevoegen aan uw toepassingen…
operation-complete = Succesvol voltooid
operation-failed = De bewerking is niet voltooid

# Metadata
meta-section-package = Pakket
meta-section-other = Overig
meta-package = Naam
meta-version = Versie
meta-format = Formaat
meta-architecture = Architectuur
meta-maintainer = Beheerder
meta-section = Sectie
meta-license = Licentie
meta-homepage = Website
meta-installed-size = Geïnstalleerde grootte
meta-file-size = Bestandsgrootte
meta-path = Bestand
meta-description = Beschrijving
version-unknown = Onbekend
meta-appimage-metadata = Metagegevens
meta-appimage-unread = Konden niet uit het bestand worden gelezen; alleen de naam en de grootte worden getoond.

# Dependency kinds
dep-pre-depends = Vereist vóór installatie
dep-depends = Vereist
dep-recommends = Aanbevolen
dep-suggests = Voorgesteld
dep-conflicts = Conflicteert met
dep-breaks = Breekt
dep-replaces = Vervangt
dep-provides = Levert

# Dependency status
dep-status-installed = Geïnstalleerd ({ $version })
dep-status-available = Beschikbaar ({ $version })
dep-status-provided-by = Geleverd door { $providers }
dep-status-missing = Niet beschikbaar
dep-status-unknown = Niet gecontroleerd
dep-alternatives = of
dependencies-none = Dit pakket declareert geen afhankelijkheden.
dependencies-bundled = AppImages bevatten alles wat ze nodig hebben, dus er hoeft niets extra’s geïnstalleerd te worden.
dependencies-flatpak = Dit bestand vermeldt geen runtime. Flatpak bepaalt bij de installatie wat er nodig is.
dependencies-resolving = Afhankelijkheden worden op uw systeem gecontroleerd…
dependencies-unsatisfiable = { $count ->
    [one] Aan { $count } vereiste afhankelijkheid kan niet worden voldaan.
   *[other] Aan { $count } vereiste afhankelijkheden kan niet worden voldaan.
  }

# Install plan
plan-title = Wat er gaat gebeuren
plan-install = Installeren
plan-upgrade = Bijwerken
plan-downgrade = Terugzetten
plan-remove = Verwijderen
plan-resolving = Er wordt bepaald wat er geïnstalleerd wordt…
plan-no-changes = Er hoeft niets te veranderen.
plan-additional = { $count ->
    [one] Er wordt { $count } extra pakket geïnstalleerd.
   *[other] Er worden { $count } extra pakketten geïnstalleerd.
  }
plan-download = { $size } te downloaden
plan-disk = { $size } schijfruimte
plan-blocked = Dit pakket kan zo niet worden geïnstalleerd:

# Files
files-none = Dit pakket installeert geen bestanden.
files-unavailable = Een Flatpak-bundel bevat geen bestandsindex, dus de inhoud kan niet worden getoond zonder alles uit te pakken.
files-hidden = De bestandenlijst staat uit in de instellingen.
files-count = { $count ->
    [one] { $count } bestand
   *[other] { $count } bestanden
  }
files-truncated = De eerste { $shown } van { $total } items worden getoond.
files-link = verwijst naar { $target }

# Supported formats
supported-formats = Ondersteunde formaten
format-deb = Debian-pakket
format-rpm = RPM-pakket
format-flatpak = Flatpak
format-appimage = AppImage
format-ready = Ondersteund
format-missing = Vereist { $tools }

# Settings
settings-appearance = Weergave
settings-theme = Thema
theme-dark = Donker
theme-light = Licht
theme-system = Bureaublad volgen
settings-behaviour = Gedrag
settings-privilege-backend = Installeren met
privilege-auto = PackageKit, of systeemgereedschap
privilege-packagekit = Alleen PackageKit
privilege-native = Alleen systeemgereedschap
settings-flatpak-scope = Flatpaks installeren voor
flatpak-scope-user = Alleen mij
flatpak-scope-system = Iedereen op deze computer
settings-show-recommends = Aanbevolen pakketten tonen
settings-show-suggests = Voorgestelde pakketten tonen
settings-show-file-list = Bestandenlijst tonen

# Confirmation
confirm-remove-title = { $name } verwijderen?
confirm-remove-body = Hiermee wordt het pakket van uw systeem verwijderd.

# About
about-repository = Repository
about-support = Probleem melden

# Errors
error-unknown-format = { $file } is geen pakketbestand dat deze toepassing kan openen.
error-unsupported-format = { $format }-bestanden vereisen { $tools }, dat niet geïnstalleerd is.
error-missing-tool = { $program } is niet geïnstalleerd.
error-command-failed = { $program } meldde een probleem: { $detail }
error-timeout = { $program } duurde te lang en is gestopt.
error-parse = Kon het pakket niet interpreteren: { $detail }
error-packagekit = PackageKit meldde een probleem: { $detail }
error-not-authorized = De verificatie is geannuleerd of geweigerd.
error-not-implemented = Ondersteuning voor { $format }-bestanden is nog niet geïmplementeerd.
