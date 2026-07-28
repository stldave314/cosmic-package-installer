# Application
app-title = Установка пакетов
app-description = Установка и просмотр файлов пакетов

# Menus
menu-file = Файл
menu-view = Вид
menu-help = Справка
open-package = Открыть пакет…
close-package = Закрыть пакет
quit = Выйти
settings = Параметры
about = О программе
reload = Обновить

# Window
no-package-title = Пакет не открыт
no-package-body = Откройте файл .deb, .rpm, .flatpak или .appimage, чтобы просмотреть и установить его.
open-package-button = Открыть пакет…
loading-package = Чтение пакета…

# Tabs
tab-details = Сведения
tab-dependencies = Зависимости
tab-files = Файлы
toggle-sidebar = Показать или скрыть боковую панель

# Installed state
state-not-installed = Не установлен
state-installed = Версия { $version } установлена
state-upgrade = Версия { $version } установлена и может быть обновлена
state-downgrade = Установленная версия { $version } новее этого файла
state-unknown = Состояние установки неизвестно

# Actions
action-install = Установить
action-reinstall = Переустановить
action-upgrade = Обновить
action-downgrade = Откатить версию
action-remove = Удалить
cancel = Отмена
dismiss = Закрыть
retry = Повторить

# Progress
progress-installing = Установка
progress-reinstalling = Переустановка
progress-upgrading = Обновление
progress-downgrading = Откат версии
progress-removing = Удаление
operation-complete = Успешно завершено
operation-failed = Операция не была завершена

# Metadata
meta-section-package = Пакет
meta-section-other = Прочее
meta-package = Имя
meta-version = Версия
meta-format = Формат
meta-architecture = Архитектура
meta-maintainer = Сопровождающий
meta-section = Раздел
meta-license = Лицензия
meta-homepage = Веб-сайт
meta-installed-size = Размер после установки
meta-file-size = Размер файла
meta-path = Файл
meta-description = Описание

# Dependency kinds
dep-pre-depends = Требуется до установки
dep-depends = Требуется
dep-recommends = Рекомендуется
dep-suggests = Предлагается
dep-conflicts = Конфликтует с
dep-breaks = Нарушает работу
dep-replaces = Заменяет
dep-provides = Предоставляет

# Dependency status
dep-status-installed = Установлен ({ $version })
dep-status-available = Доступен ({ $version })
dep-status-provided-by = Предоставляется пакетом { $providers }
dep-status-missing = Недоступен
dep-status-unknown = Не проверено
dep-alternatives = или
dependencies-none = Этот пакет не объявляет зависимостей.
dependencies-resolving = Проверка зависимостей в системе…
dependencies-unsatisfiable = { $count ->
    [one] { $count } обязательная зависимость не может быть удовлетворена.
    [few] { $count } обязательные зависимости не могут быть удовлетворены.
    [many] { $count } обязательных зависимостей не могут быть удовлетворены.
   *[other] { $count } обязательной зависимости не могут быть удовлетворены.
  }

# Install plan
plan-title = Что произойдёт
plan-install = Установить
plan-upgrade = Обновить
plan-downgrade = Откатить
plan-remove = Удалить
plan-resolving = Определение того, что будет установлено…
plan-no-changes = Ничего менять не нужно.
plan-additional = { $count ->
    [one] Будет установлен { $count } дополнительный пакет.
    [few] Будут установлены { $count } дополнительных пакета.
    [many] Будет установлено { $count } дополнительных пакетов.
   *[other] Будет установлено { $count } дополнительных пакетов.
  }
plan-download = { $size } для загрузки
plan-disk = { $size } на диске
plan-blocked = Этот пакет невозможно установить в текущем состоянии:

# Files
files-none = Этот пакет не устанавливает файлов.
files-hidden = Список файлов отключён в параметрах.
files-count = { $count ->
    [one] { $count } файл
    [few] { $count } файла
    [many] { $count } файлов
   *[other] { $count } файла
  }
files-truncated = Показаны первые { $shown } из { $total } записей.
files-link = ссылается на { $target }

# Supported formats
supported-formats = Поддерживаемые форматы
format-deb = Пакет Debian
format-rpm = Пакет RPM
format-flatpak = Flatpak
format-appimage = AppImage
format-ready = Поддерживается
format-missing = Требуется { $tools }

# Settings
settings-appearance = Внешний вид
settings-theme = Тема
theme-dark = Тёмная
theme-light = Светлая
theme-system = Как в системе
settings-behaviour = Поведение
settings-privilege-backend = Устанавливать через
privilege-auto = PackageKit или системные средства
privilege-packagekit = Только PackageKit
privilege-native = Только системные средства
settings-show-recommends = Показывать рекомендуемые пакеты
settings-show-suggests = Показывать предлагаемые пакеты
settings-show-file-list = Показывать список файлов

# Confirmation
confirm-remove-title = Удалить { $name }?
confirm-remove-body = Пакет будет удалён из системы.

# About
about-repository = Репозиторий
about-support = Сообщить о проблеме

# Errors
error-unknown-format = { $file } не является файлом пакета, который может открыть эта программа.
error-unsupported-format = Для файлов { $format } требуется { $tools }, который не установлен.
error-missing-tool = { $program } не установлен.
error-command-failed = { $program } сообщил о проблеме: { $detail }
error-timeout = { $program } работал слишком долго и был остановлен.
error-parse = Не удалось разобрать пакет: { $detail }
error-packagekit = PackageKit сообщил о проблеме: { $detail }
error-not-authorized = Аутентификация отменена или отклонена.
error-not-implemented = Поддержка файлов { $format } ещё не реализована.
