# Application
app-title = Instalator pakietów
app-description = Instalowanie i przeglądanie plików pakietów

# Menus
menu-file = Plik
menu-view = Widok
menu-help = Pomoc
open-package = Otwórz pakiet…
close-package = Zamknij pakiet
quit = Zakończ
settings = Ustawienia
about = O programie
reload = Odśwież

# Window
no-package-title = Nie otwarto żadnego pakietu
no-package-body = Otwórz plik .deb, .rpm, .flatpak lub .appimage, aby go przejrzeć i zainstalować.
open-package-button = Otwórz pakiet…
loading-package = Odczytywanie pakietu…

# Tabs
tab-details = Szczegóły
tab-dependencies = Zależności
tab-files = Pliki
toggle-sidebar = Pokaż lub ukryj panel boczny

# Installed state
state-not-installed = Niezainstalowany
state-installed = Wersja { $version } jest zainstalowana
state-upgrade = Wersja { $version } jest zainstalowana i można ją zaktualizować
state-downgrade = Zainstalowana wersja { $version } jest nowsza niż ten plik
state-unknown = Nieznany stan instalacji

# Actions
action-install = Zainstaluj
action-reinstall = Zainstaluj ponownie
action-upgrade = Zaktualizuj
action-downgrade = Przywróć starszą wersję
action-remove = Odinstaluj
cancel = Anuluj
dismiss = Zamknij
retry = Spróbuj ponownie

# Progress
progress-installing = Instalowanie
progress-reinstalling = Ponowne instalowanie
progress-upgrading = Aktualizowanie
progress-downgrading = Przywracanie starszej wersji
progress-removing = Odinstalowywanie
operation-complete = Zakończono pomyślnie
operation-failed = Operacja nie została ukończona

# Metadata
meta-section-package = Pakiet
meta-section-other = Inne
meta-package = Nazwa
meta-version = Wersja
meta-format = Format
meta-architecture = Architektura
meta-maintainer = Opiekun
meta-section = Sekcja
meta-license = Licencja
meta-homepage = Strona internetowa
meta-installed-size = Rozmiar po instalacji
meta-file-size = Rozmiar pliku
meta-path = Plik
meta-description = Opis

# Dependency kinds
dep-pre-depends = Wymagane przed instalacją
dep-depends = Wymagane
dep-recommends = Zalecane
dep-suggests = Sugerowane
dep-conflicts = Konflikt z
dep-breaks = Psuje
dep-replaces = Zastępuje
dep-provides = Dostarcza

# Dependency status
dep-status-installed = Zainstalowany ({ $version })
dep-status-available = Dostępny ({ $version })
dep-status-provided-by = Dostarczany przez { $providers }
dep-status-missing = Niedostępny
dep-status-unknown = Niesprawdzony
dep-alternatives = lub
dependencies-none = Ten pakiet nie deklaruje żadnych zależności.
dependencies-resolving = Sprawdzanie zależności w systemie…
dependencies-unsatisfiable = { $count ->
    [one] { $count } wymagana zależność nie może zostać spełniona.
    [few] { $count } wymagane zależności nie mogą zostać spełnione.
    [many] { $count } wymaganych zależności nie może zostać spełnionych.
   *[other] { $count } wymaganej zależności nie można spełnić.
  }

# Install plan
plan-title = Co się wydarzy
plan-install = Zainstaluj
plan-upgrade = Zaktualizuj
plan-downgrade = Przywróć starszą wersję
plan-remove = Usuń
plan-resolving = Ustalanie, co zostanie zainstalowane…
plan-no-changes = Nic nie wymaga zmiany.
plan-additional = { $count ->
    [one] Zostanie zainstalowany { $count } dodatkowy pakiet.
    [few] Zostaną zainstalowane { $count } dodatkowe pakiety.
    [many] Zostanie zainstalowanych { $count } dodatkowych pakietów.
   *[other] Zostanie zainstalowanych { $count } dodatkowych pakietów.
  }
plan-download = { $size } do pobrania
plan-disk = { $size } miejsca na dysku
plan-blocked = Tego pakietu nie można zainstalować w obecnym stanie:

# Files
files-none = Ten pakiet nie instaluje żadnych plików.
files-hidden = Lista plików jest wyłączona w ustawieniach.
files-count = { $count ->
    [one] { $count } plik
    [few] { $count } pliki
    [many] { $count } plików
   *[other] { $count } pliku
  }
files-truncated = Wyświetlono pierwsze { $shown } z { $total } wpisów.
files-link = wskazuje na { $target }

# Supported formats
supported-formats = Obsługiwane formaty
format-deb = Pakiet Debiana
format-rpm = Pakiet RPM
format-flatpak = Flatpak
format-appimage = AppImage
format-ready = Obsługiwany
format-missing = Wymaga { $tools }

# Settings
settings-appearance = Wygląd
settings-theme = Motyw
theme-dark = Ciemny
theme-light = Jasny
theme-system = Zgodnie z pulpitem
settings-behaviour = Zachowanie
settings-privilege-backend = Instaluj przy użyciu
privilege-auto = PackageKit lub narzędzi systemowych
privilege-packagekit = Tylko PackageKit
privilege-native = Tylko narzędzia systemowe
settings-show-recommends = Pokaż zalecane pakiety
settings-show-suggests = Pokaż sugerowane pakiety
settings-show-file-list = Pokaż listę plików

# Confirmation
confirm-remove-title = Odinstalować { $name }?
confirm-remove-body = Spowoduje to usunięcie pakietu z systemu.

# About
about-repository = Repozytorium
about-support = Zgłoś problem

# Errors
error-unknown-format = { $file } nie jest plikiem pakietu, który ten program potrafi otworzyć.
error-unsupported-format = Pliki { $format } wymagają { $tools }, które nie jest zainstalowane.
error-missing-tool = { $program } nie jest zainstalowany.
error-command-failed = { $program } zgłosił problem: { $detail }
error-timeout = { $program } działał zbyt długo i został zatrzymany.
error-parse = Nie udało się zinterpretować pakietu: { $detail }
error-packagekit = PackageKit zgłosił problem: { $detail }
error-not-authorized = Uwierzytelnianie zostało anulowane lub odrzucone.
error-not-implemented = Obsługa plików { $format } nie została jeszcze zaimplementowana.
