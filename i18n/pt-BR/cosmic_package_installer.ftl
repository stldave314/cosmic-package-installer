# Application
app-title = Instalador de pacotes
app-description = Instalar e inspecionar arquivos de pacote

# Menus
menu-file = Arquivo
menu-view = Exibir
menu-help = Ajuda
open-package = Abrir pacote…
close-package = Fechar pacote
quit = Sair
settings = Configurações
about = Sobre
reload = Recarregar

# Window
no-package-title = Nenhum pacote aberto
no-package-body = Abra um arquivo .deb, .rpm, .flatpak ou .appimage para inspecioná-lo e instalá-lo.
open-package-button = Abrir pacote…
loading-package = Lendo o pacote…

# Tabs
tab-details = Detalhes
tab-dependencies = Dependências
tab-files = Arquivos
toggle-sidebar = Mostrar ou ocultar a barra lateral

# Installed state
state-not-installed = Não instalado
state-installed = A versão { $version } está instalada
state-upgrade = A versão { $version } está instalada e pode ser atualizada
state-downgrade = A versão { $version } instalada é mais recente que este arquivo
state-unknown = Estado de instalação desconhecido

# Actions
action-install = Instalar
action-reinstall = Reinstalar
action-upgrade = Atualizar
action-downgrade = Reverter versão
action-remove = Desinstalar
cancel = Cancelar
dismiss = Fechar
retry = Tentar novamente

# Progress
progress-installing = Instalando
progress-reinstalling = Reinstalando
progress-upgrading = Atualizando
progress-downgrading = Revertendo versão
progress-removing = Desinstalando
progress-copying = Copiando o AppImage…
progress-integrating = Adicionando aos seus aplicativos…
operation-complete = Concluído com sucesso
operation-failed = A operação não foi concluída

# Metadata
meta-section-package = Pacote
meta-section-other = Outros
meta-package = Nome
meta-version = Versão
meta-format = Formato
meta-architecture = Arquitetura
meta-maintainer = Mantenedor
meta-section = Seção
meta-license = Licença
meta-homepage = Site
meta-installed-size = Tamanho instalado
meta-file-size = Tamanho do arquivo
meta-path = Arquivo
meta-description = Descrição
version-unknown = Desconhecida
meta-appimage-metadata = Metadados
meta-appimage-unread = Não foi possível lê-los do arquivo, portanto apenas o nome e o tamanho são exibidos.

# Dependency kinds
dep-pre-depends = Necessário antes da instalação
dep-depends = Necessário
dep-recommends = Recomendado
dep-suggests = Sugerido
dep-conflicts = Conflita com
dep-breaks = Quebra
dep-replaces = Substitui
dep-provides = Fornece

# Dependency status
dep-status-installed = Instalado ({ $version })
dep-status-available = Disponível ({ $version })
dep-status-provided-by = Fornecido por { $providers }
dep-status-missing = Indisponível
dep-status-unknown = Não verificado
dep-alternatives = ou
dependencies-none = Este pacote não declara dependências.
dependencies-bundled = Os AppImages incluem tudo o que precisam, portanto não há mais nada a instalar.
dependencies-flatpak = Este arquivo não declara nenhum runtime. O Flatpak determina o que é necessário ao instalá-lo.
dependencies-resolving = Verificando as dependências no seu sistema…
dependencies-unsatisfiable = { $count ->
    [one] { $count } dependência necessária não pode ser satisfeita.
   *[other] { $count } dependências necessárias não podem ser satisfeitas.
  }

# Install plan
plan-title = O que vai acontecer
plan-install = Instalar
plan-upgrade = Atualizar
plan-downgrade = Reverter
plan-remove = Remover
plan-resolving = Calculando o que será instalado…
plan-no-changes = Nada precisa mudar.
plan-additional = { $count ->
    [one] { $count } pacote adicional será instalado.
   *[other] { $count } pacotes adicionais serão instalados.
  }
plan-download = { $size } para baixar
plan-disk = { $size } de espaço em disco
plan-blocked = Este pacote não pode ser instalado como está:

# Files
files-none = Este pacote não instala nenhum arquivo.
files-unavailable = Um pacote Flatpak não traz um índice de arquivos, portanto seu conteúdo não pode ser listado sem descompactá-lo por inteiro.
files-hidden = A lista de arquivos está desativada nas configurações.
files-count = { $count ->
    [one] { $count } arquivo
   *[other] { $count } arquivos
  }
files-truncated = Mostrando as primeiras { $shown } de { $total } entradas.
files-link = aponta para { $target }

# Supported formats
supported-formats = Formatos compatíveis
format-deb = Pacote Debian
format-rpm = Pacote RPM
format-flatpak = Flatpak
format-appimage = AppImage
format-ready = Compatível
format-missing = Requer { $tools }

# Settings
settings-appearance = Aparência
settings-theme = Tema
theme-dark = Escuro
theme-light = Claro
theme-system = Acompanhar a área de trabalho
settings-behaviour = Comportamento
settings-privilege-backend = Instalar usando
privilege-auto = PackageKit ou ferramentas do sistema
privilege-packagekit = Apenas PackageKit
privilege-native = Apenas ferramentas do sistema
settings-flatpak-scope = Instalar Flatpaks para
flatpak-scope-user = Somente eu
flatpak-scope-system = Todos neste computador
settings-show-recommends = Mostrar pacotes recomendados
settings-show-suggests = Mostrar pacotes sugeridos
settings-show-file-list = Mostrar a lista de arquivos

# Confirmation
confirm-remove-title = Desinstalar { $name }?
confirm-remove-body = Isto removerá o pacote do seu sistema.

# About
about-repository = Repositório
about-support = Relatar um problema

# Errors
error-unknown-format = { $file } não é um arquivo de pacote que este aplicativo possa abrir.
error-unsupported-format = Arquivos { $format } precisam de { $tools }, que não está instalado.
error-missing-tool = { $program } não está instalado.
error-command-failed = { $program } relatou um problema: { $detail }
error-timeout = { $program } demorou demais e foi interrompido.
error-parse = Não foi possível interpretar o pacote: { $detail }
error-packagekit = O PackageKit relatou um problema: { $detail }
error-not-authorized = A autenticação foi cancelada ou recusada.
error-not-implemented = O suporte a arquivos { $format } ainda não foi implementado.
