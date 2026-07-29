# Application
app-title = Installazione pacchetti
app-description = Installa ed esamina file di pacchetto

# Menus
menu-file = File
menu-view = Visualizza
menu-help = Aiuto
open-package = Apri pacchetto…
close-package = Chiudi pacchetto
quit = Esci
settings = Impostazioni
about = Informazioni
reload = Ricarica

# Window
no-package-title = Nessun pacchetto aperto
no-package-body = Apri un file .deb, .rpm, .flatpak o .appimage per esaminarlo e installarlo.
open-package-button = Apri pacchetto…
loading-package = Lettura del pacchetto…

# Tabs
tab-details = Dettagli
tab-dependencies = Dipendenze
tab-files = File
toggle-sidebar = Mostra o nascondi la barra laterale

# Installed state
state-not-installed = Non installato
state-installed = La versione { $version } è installata
state-upgrade = La versione { $version } è installata e può essere aggiornata
state-downgrade = La versione { $version } installata è più recente di questo file
state-unknown = Stato di installazione sconosciuto

# Actions
action-install = Installa
action-reinstall = Reinstalla
action-upgrade = Aggiorna
action-downgrade = Torna alla versione precedente
action-remove = Disinstalla
cancel = Annulla
dismiss = Chiudi
retry = Riprova

# Progress
progress-installing = Installazione
progress-reinstalling = Reinstallazione
progress-upgrading = Aggiornamento
progress-downgrading = Ripristino versione precedente
progress-removing = Disinstallazione
progress-copying = Copia dell’AppImage in corso…
progress-integrating = Aggiunta alle tue applicazioni…
operation-complete = Completato correttamente
operation-failed = L'operazione non è stata completata

# Metadata
meta-section-package = Pacchetto
meta-section-other = Altro
meta-package = Nome
meta-version = Versione
meta-format = Formato
meta-architecture = Architettura
meta-maintainer = Manutentore
meta-section = Sezione
meta-license = Licenza
meta-homepage = Sito web
meta-installed-size = Dimensione installata
meta-file-size = Dimensione del file
meta-path = File
meta-description = Descrizione
version-unknown = Sconosciuta
meta-appimage-metadata = Metadati
meta-appimage-unread = Non è stato possibile leggerli dal file, quindi vengono mostrati solo nome e dimensione.

# Dependency kinds
dep-pre-depends = Necessario prima dell'installazione
dep-depends = Necessario
dep-recommends = Consigliato
dep-suggests = Suggerito
dep-conflicts = In conflitto con
dep-breaks = Rende inutilizzabile
dep-replaces = Sostituisce
dep-provides = Fornisce

# Dependency status
dep-status-installed = Installato ({ $version })
dep-status-available = Disponibile ({ $version })
dep-status-provided-by = Fornito da { $providers }
dep-status-missing = Non disponibile
dep-status-unknown = Non verificato
dep-alternatives = oppure
dependencies-none = Questo pacchetto non dichiara dipendenze.
dependencies-bundled = Le AppImage includono tutto ciò di cui hanno bisogno, quindi non c’è altro da installare.
dependencies-flatpak = Questo file non dichiara alcun runtime. Flatpak stabilisce ciò che serve al momento dell’installazione.
dependencies-resolving = Verifica delle dipendenze sul sistema…
dependencies-unsatisfiable = { $count ->
    [one] { $count } dipendenza necessaria non può essere soddisfatta.
   *[other] { $count } dipendenze necessarie non possono essere soddisfatte.
  }

# Install plan
plan-title = Cosa succederà
plan-install = Installa
plan-upgrade = Aggiorna
plan-downgrade = Torna indietro
plan-remove = Rimuovi
plan-resolving = Calcolo di ciò che verrà installato…
plan-no-changes = Non serve modificare nulla.
plan-additional = { $count ->
    [one] Verrà installato { $count } pacchetto aggiuntivo.
   *[other] Verranno installati { $count } pacchetti aggiuntivi.
  }
plan-download = { $size } da scaricare
plan-disk = { $size } di spazio su disco
plan-blocked = Questo pacchetto non può essere installato così com'è:

# Files
files-none = Questo pacchetto non installa alcun file.
files-unavailable = Un bundle Flatpak non contiene un indice dei file, quindi non è possibile elencarne il contenuto senza estrarlo per intero.
files-hidden = L'elenco dei file è disattivato nelle impostazioni.
files-count = { $count ->
    [one] { $count } file
   *[other] { $count } file
  }
files-truncated = Vengono mostrate le prime { $shown } voci su { $total }.
files-link = punta a { $target }

# Supported formats
supported-formats = Formati supportati
format-deb = Pacchetto Debian
format-rpm = Pacchetto RPM
format-flatpak = Flatpak
format-appimage = AppImage
format-ready = Supportato
format-missing = Richiede { $tools }

# Settings
settings-appearance = Aspetto
settings-theme = Tema
theme-dark = Scuro
theme-light = Chiaro
theme-system = Segui il desktop
settings-behaviour = Comportamento
settings-privilege-backend = Installa con
privilege-auto = PackageKit o gli strumenti di sistema
privilege-packagekit = Solo PackageKit
privilege-native = Solo strumenti di sistema
settings-flatpak-scope = Installa i Flatpak per
flatpak-scope-user = Solo me
flatpak-scope-system = Tutti gli utenti di questo computer
settings-show-recommends = Mostra i pacchetti consigliati
settings-show-suggests = Mostra i pacchetti suggeriti
settings-show-file-list = Mostra l'elenco dei file

# Confirmation
confirm-remove-title = Disinstallare { $name }?
confirm-remove-body = Il pacchetto verrà rimosso dal sistema.

# About
about-repository = Repository
about-support = Segnala un problema

# Errors
error-unknown-format = { $file } non è un file di pacchetto che questa applicazione possa aprire.
error-unsupported-format = I file { $format } richiedono { $tools }, che non è installato.
error-missing-tool = { $program } non è installato.
error-command-failed = { $program } ha segnalato un problema: { $detail }
error-timeout = { $program } ha impiegato troppo tempo ed è stato interrotto.
error-parse = Impossibile interpretare il pacchetto: { $detail }
error-packagekit = PackageKit ha segnalato un problema: { $detail }
error-not-authorized = L'autenticazione è stata annullata o rifiutata.
error-not-implemented = Il supporto per i file { $format } non è ancora implementato.
