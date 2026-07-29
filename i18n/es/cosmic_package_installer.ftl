# Application
app-title = Instalador de paquetes
app-description = Instalar e inspeccionar archivos de paquete

# Menus
menu-file = Archivo
menu-view = Ver
menu-help = Ayuda
open-package = Abrir paquete…
close-package = Cerrar paquete
quit = Salir
settings = Ajustes
about = Acerca de
reload = Recargar

# Window
no-package-title = Ningún paquete abierto
no-package-body = Abra un archivo .deb, .rpm, .flatpak o .appimage para inspeccionarlo e instalarlo.
open-package-button = Abrir paquete…
loading-package = Leyendo el paquete…

# Tabs
tab-details = Detalles
tab-dependencies = Dependencias
tab-files = Archivos
toggle-sidebar = Mostrar u ocultar la barra lateral

# Installed state
state-not-installed = No instalado
state-installed = La versión { $version } está instalada
state-upgrade = La versión { $version } está instalada y se puede actualizar
state-downgrade = La versión { $version } instalada es más reciente que este archivo
state-unknown = Estado de instalación desconocido

# Actions
action-install = Instalar
action-reinstall = Reinstalar
action-upgrade = Actualizar
action-downgrade = Volver a versión anterior
action-remove = Desinstalar
cancel = Cancelar
dismiss = Cerrar
retry = Reintentar

# Progress
progress-installing = Instalando
progress-reinstalling = Reinstalando
progress-upgrading = Actualizando
progress-downgrading = Volviendo a versión anterior
progress-removing = Desinstalando
progress-copying = Copiando la AppImage en su sitio…
progress-integrating = Añadiéndola a tus aplicaciones…
operation-complete = Finalizado correctamente
operation-failed = La operación no se completó

# Metadata
meta-section-package = Paquete
meta-section-other = Otros
meta-package = Nombre
meta-version = Versión
meta-format = Formato
meta-architecture = Arquitectura
meta-maintainer = Responsable
meta-section = Sección
meta-license = Licencia
meta-homepage = Sitio web
meta-installed-size = Tamaño instalado
meta-file-size = Tamaño del archivo
meta-path = Archivo
meta-description = Descripción
version-unknown = Desconocida
meta-appimage-metadata = Metadatos
meta-appimage-unread = No se han podido leer del archivo, así que solo se muestran su nombre y su tamaño.

# Dependency kinds
dep-pre-depends = Necesario antes de la instalación
dep-depends = Necesario
dep-recommends = Recomendado
dep-suggests = Sugerido
dep-conflicts = Entra en conflicto con
dep-breaks = Rompe
dep-replaces = Sustituye a
dep-provides = Proporciona

# Dependency status
dep-status-installed = Instalado ({ $version })
dep-status-available = Disponible ({ $version })
dep-status-provided-by = Proporcionado por { $providers }
dep-status-missing = No disponible
dep-status-unknown = Sin comprobar
dep-alternatives = o
dependencies-none = Este paquete no declara dependencias.
dependencies-bundled = Las AppImage incluyen todo lo que necesitan, así que no hay nada más que instalar.
dependencies-flatpak = Este archivo no declara ningún entorno de ejecución. Flatpak determina lo que hace falta al instalarlo.
dependencies-resolving = Comprobando las dependencias en su sistema…
dependencies-unsatisfiable = { $count ->
    [one] No se puede satisfacer { $count } dependencia necesaria.
   *[other] No se pueden satisfacer { $count } dependencias necesarias.
  }

# Install plan
plan-title = Qué va a ocurrir
plan-install = Instalar
plan-upgrade = Actualizar
plan-downgrade = Volver atrás
plan-remove = Eliminar
plan-resolving = Calculando qué se va a instalar…
plan-no-changes = No hace falta cambiar nada.
plan-additional = { $count ->
    [one] Se instalará { $count } paquete adicional.
   *[other] Se instalarán { $count } paquetes adicionales.
  }
plan-download = { $size } para descargar
plan-disk = { $size } de espacio en disco
plan-blocked = Este paquete no se puede instalar tal como está:

# Files
files-none = Este paquete no instala ningún archivo.
files-unavailable = Un paquete Flatpak no incluye un índice de archivos, así que su contenido no se puede listar sin descomprimirlo entero.
files-hidden = La lista de archivos está desactivada en los ajustes.
files-count = { $count ->
    [one] { $count } archivo
   *[other] { $count } archivos
  }
files-truncated = Mostrando las primeras { $shown } de { $total } entradas.
files-link = enlaza a { $target }

# Supported formats
supported-formats = Formatos admitidos
format-deb = Paquete Debian
format-rpm = Paquete RPM
format-flatpak = Flatpak
format-appimage = AppImage
format-ready = Admitido
format-missing = Necesita { $tools }

# Settings
settings-appearance = Apariencia
settings-theme = Tema
theme-dark = Oscuro
theme-light = Claro
theme-system = Seguir al escritorio
settings-behaviour = Comportamiento
settings-privilege-backend = Instalar con
privilege-auto = PackageKit o las herramientas del sistema
privilege-packagekit = Solo PackageKit
privilege-native = Solo herramientas del sistema
settings-flatpak-scope = Instalar Flatpaks para
flatpak-scope-user = Solo para mí
flatpak-scope-system = Todos los usuarios de este equipo
settings-show-recommends = Mostrar paquetes recomendados
settings-show-suggests = Mostrar paquetes sugeridos
settings-show-file-list = Mostrar la lista de archivos

# Confirmation
confirm-remove-title = ¿Desinstalar { $name }?
confirm-remove-body = Esto eliminará el paquete de su sistema.

# About
about-repository = Repositorio
about-support = Informar de un problema

# Errors
error-unknown-format = { $file } no es un archivo de paquete que esta aplicación pueda abrir.
error-unsupported-format = Los archivos { $format } necesitan { $tools }, que no está instalado.
error-missing-tool = { $program } no está instalado.
error-command-failed = { $program } informó de un problema: { $detail }
error-timeout = { $program } tardó demasiado y se detuvo.
error-parse = No se pudo interpretar el paquete: { $detail }
error-packagekit = PackageKit informó de un problema: { $detail }
error-not-authorized = La autenticación se canceló o se denegó.
error-not-implemented = La compatibilidad con archivos { $format } aún no está implementada.
