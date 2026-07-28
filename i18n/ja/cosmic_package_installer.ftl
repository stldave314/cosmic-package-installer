# Application
app-title = パッケージインストーラー
app-description = パッケージファイルのインストールと確認

# Menus
menu-file = ファイル
menu-view = 表示
menu-help = ヘルプ
open-package = パッケージを開く…
close-package = パッケージを閉じる
quit = 終了
settings = 設定
about = このアプリについて
reload = 再読み込み

# Window
no-package-title = パッケージが開かれていません
no-package-body = .deb、.rpm、.flatpak、.appimage ファイルを開くと、内容を確認してインストールできます。
open-package-button = パッケージを開く…
loading-package = パッケージを読み込んでいます…

# Tabs
tab-details = 詳細
tab-dependencies = 依存関係
tab-files = ファイル
toggle-sidebar = サイドバーの表示・非表示

# Installed state
state-not-installed = 未インストール
state-installed = バージョン { $version } がインストールされています
state-upgrade = バージョン { $version } がインストールされており、更新できます
state-downgrade = インストール済みのバージョン { $version } はこのファイルより新しいです
state-unknown = インストール状態が不明です

# Actions
action-install = インストール
action-reinstall = 再インストール
action-upgrade = 更新
action-downgrade = ダウングレード
action-remove = アンインストール
cancel = キャンセル
dismiss = 閉じる
retry = 再試行

# Progress
progress-installing = インストール中
progress-reinstalling = 再インストール中
progress-upgrading = 更新中
progress-downgrading = ダウングレード中
progress-removing = アンインストール中
operation-complete = 正常に完了しました
operation-failed = 操作は完了しませんでした

# Metadata
meta-section-package = パッケージ
meta-section-other = その他
meta-package = 名前
meta-version = バージョン
meta-format = 形式
meta-architecture = アーキテクチャ
meta-maintainer = メンテナー
meta-section = セクション
meta-license = ライセンス
meta-homepage = ウェブサイト
meta-installed-size = インストール後のサイズ
meta-file-size = ファイルサイズ
meta-path = ファイル
meta-description = 説明

# Dependency kinds
dep-pre-depends = インストール前に必要
dep-depends = 必須
dep-recommends = 推奨
dep-suggests = 提案
dep-conflicts = 競合
dep-breaks = 破損させる
dep-replaces = 置き換え
dep-provides = 提供

# Dependency status
dep-status-installed = インストール済み ({ $version })
dep-status-available = 入手可能 ({ $version })
dep-status-provided-by = { $providers } が提供
dep-status-missing = 入手できません
dep-status-unknown = 未確認
dep-alternatives = または
dependencies-none = このパッケージは依存関係を宣言していません。
dependencies-resolving = システム上の依存関係を確認しています…
dependencies-unsatisfiable = { $count ->
   *[other] 必須の依存関係 { $count } 件を満たせません。
  }

# Install plan
plan-title = 実行される内容
plan-install = インストール
plan-upgrade = 更新
plan-downgrade = ダウングレード
plan-remove = 削除
plan-resolving = インストールされる内容を確認しています…
plan-no-changes = 変更の必要はありません。
plan-additional = { $count ->
   *[other] 追加で { $count } 個のパッケージがインストールされます。
  }
plan-download = ダウンロード { $size }
plan-disk = ディスク容量 { $size }
plan-blocked = このパッケージは現在の状態ではインストールできません:

# Files
files-none = このパッケージはファイルをインストールしません。
files-hidden = ファイル一覧は設定で無効になっています。
files-count = { $count ->
   *[other] { $count } 個のファイル
  }
files-truncated = { $total } 件のうち最初の { $shown } 件を表示しています。
files-link = { $target } へのリンク

# Supported formats
supported-formats = 対応形式
format-deb = Debian パッケージ
format-rpm = RPM パッケージ
format-flatpak = Flatpak
format-appimage = AppImage
format-ready = 対応
format-missing = { $tools } が必要です

# Settings
settings-appearance = 外観
settings-theme = テーマ
theme-dark = ダーク
theme-light = ライト
theme-system = デスクトップに合わせる
settings-behaviour = 動作
settings-privilege-backend = インストール方法
privilege-auto = PackageKit またはシステムツール
privilege-packagekit = PackageKit のみ
privilege-native = システムツールのみ
settings-show-recommends = 推奨パッケージを表示
settings-show-suggests = 提案パッケージを表示
settings-show-file-list = ファイル一覧を表示

# Confirmation
confirm-remove-title = { $name } をアンインストールしますか?
confirm-remove-body = このパッケージをシステムから削除します。

# About
about-repository = リポジトリ
about-support = 問題を報告

# Errors
error-unknown-format = { $file } はこのアプリで開けるパッケージファイルではありません。
error-unsupported-format = { $format } ファイルには { $tools } が必要ですが、インストールされていません。
error-missing-tool = { $program } がインストールされていません。
error-command-failed = { $program } が問題を報告しました: { $detail }
error-timeout = { $program } の実行に時間がかかりすぎたため停止しました。
error-parse = パッケージを解釈できませんでした: { $detail }
error-packagekit = PackageKit が問題を報告しました: { $detail }
error-not-authorized = 認証がキャンセルまたは拒否されました。
error-not-implemented = { $format } ファイルへの対応はまだ実装されていません。
