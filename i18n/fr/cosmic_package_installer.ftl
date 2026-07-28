# Application
app-title = Installateur de paquets
app-description = Installer et examiner des fichiers de paquet

# Menus
menu-file = Fichier
menu-view = Affichage
menu-help = Aide
open-package = Ouvrir un paquet…
close-package = Fermer le paquet
quit = Quitter
settings = Paramètres
about = À propos
reload = Recharger

# Window
no-package-title = Aucun paquet ouvert
no-package-body = Ouvrez un fichier .deb, .rpm, .flatpak ou .appimage pour l'examiner et l'installer.
open-package-button = Ouvrir un paquet…
loading-package = Lecture du paquet…

# Tabs
tab-details = Détails
tab-dependencies = Dépendances
tab-files = Fichiers
toggle-sidebar = Afficher ou masquer la barre latérale

# Installed state
state-not-installed = Non installé
state-installed = La version { $version } est installée
state-upgrade = La version { $version } est installée et peut être mise à jour
state-downgrade = La version { $version } installée est plus récente que ce fichier
state-unknown = État d'installation inconnu

# Actions
action-install = Installer
action-reinstall = Réinstaller
action-upgrade = Mettre à jour
action-downgrade = Revenir en arrière
action-remove = Désinstaller
cancel = Annuler
dismiss = Fermer
retry = Réessayer

# Progress
progress-installing = Installation
progress-reinstalling = Réinstallation
progress-upgrading = Mise à jour
progress-downgrading = Retour à une version antérieure
progress-removing = Désinstallation
operation-complete = Terminé avec succès
operation-failed = L'opération ne s'est pas terminée

# Metadata
meta-section-package = Paquet
meta-section-other = Autres
meta-package = Nom
meta-version = Version
meta-format = Format
meta-architecture = Architecture
meta-maintainer = Mainteneur
meta-section = Section
meta-license = Licence
meta-homepage = Site web
meta-installed-size = Taille installée
meta-file-size = Taille du fichier
meta-path = Fichier
meta-description = Description

# Dependency kinds
dep-pre-depends = Requis avant l'installation
dep-depends = Requis
dep-recommends = Recommandé
dep-suggests = Suggéré
dep-conflicts = En conflit avec
dep-breaks = Casse
dep-replaces = Remplace
dep-provides = Fournit

# Dependency status
dep-status-installed = Installé ({ $version })
dep-status-available = Disponible ({ $version })
dep-status-provided-by = Fourni par { $providers }
dep-status-missing = Non disponible
dep-status-unknown = Non vérifié
dep-alternatives = ou
dependencies-none = Ce paquet ne déclare aucune dépendance.
dependencies-resolving = Vérification des dépendances sur votre système…
dependencies-unsatisfiable = { $count ->
    [one] { $count } dépendance requise ne peut pas être satisfaite.
   *[other] { $count } dépendances requises ne peuvent pas être satisfaites.
  }

# Install plan
plan-title = Ce qui va se passer
plan-install = Installer
plan-upgrade = Mettre à jour
plan-downgrade = Revenir en arrière
plan-remove = Supprimer
plan-resolving = Détermination de ce qui sera installé…
plan-no-changes = Aucun changement nécessaire.
plan-additional = { $count ->
    [one] { $count } paquet supplémentaire sera installé.
   *[other] { $count } paquets supplémentaires seront installés.
  }
plan-download = { $size } à télécharger
plan-disk = { $size } d'espace disque
plan-blocked = Ce paquet ne peut pas être installé en l'état :

# Files
files-none = Ce paquet n'installe aucun fichier.
files-hidden = La liste des fichiers est désactivée dans les paramètres.
files-count = { $count ->
    [one] { $count } fichier
   *[other] { $count } fichiers
  }
files-truncated = Affichage des { $shown } premières entrées sur { $total }.
files-link = pointe vers { $target }

# Supported formats
supported-formats = Formats pris en charge
format-deb = Paquet Debian
format-rpm = Paquet RPM
format-flatpak = Flatpak
format-appimage = AppImage
format-ready = Pris en charge
format-missing = Nécessite { $tools }

# Settings
settings-appearance = Apparence
settings-theme = Thème
theme-dark = Sombre
theme-light = Clair
theme-system = Suivre le bureau
settings-behaviour = Comportement
settings-privilege-backend = Installer avec
privilege-auto = PackageKit, ou les outils système
privilege-packagekit = PackageKit uniquement
privilege-native = Outils système uniquement
settings-show-recommends = Afficher les paquets recommandés
settings-show-suggests = Afficher les paquets suggérés
settings-show-file-list = Afficher la liste des fichiers

# Confirmation
confirm-remove-title = Désinstaller { $name } ?
confirm-remove-body = Le paquet sera supprimé de votre système.

# About
about-repository = Dépôt
about-support = Signaler un problème

# Errors
error-unknown-format = { $file } n'est pas un fichier de paquet que cette application peut ouvrir.
error-unsupported-format = Les fichiers { $format } nécessitent { $tools }, qui n'est pas installé.
error-missing-tool = { $program } n'est pas installé.
error-command-failed = { $program } a signalé un problème : { $detail }
error-timeout = { $program } a mis trop de temps et a été arrêté.
error-parse = Impossible d'interpréter le paquet : { $detail }
error-packagekit = PackageKit a signalé un problème : { $detail }
error-not-authorized = L'authentification a été annulée ou refusée.
error-not-implemented = La prise en charge des fichiers { $format } n'est pas encore implémentée.
