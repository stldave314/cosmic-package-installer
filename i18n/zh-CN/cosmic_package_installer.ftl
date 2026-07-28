# Application
app-title = 软件包安装器
app-description = 安装和查看软件包文件

# Menus
menu-file = 文件
menu-view = 视图
menu-help = 帮助
open-package = 打开软件包…
close-package = 关闭软件包
quit = 退出
settings = 设置
about = 关于
reload = 重新载入

# Window
no-package-title = 未打开软件包
no-package-body = 打开 .deb、.rpm、.flatpak 或 .appimage 文件即可查看并安装。
open-package-button = 打开软件包…
loading-package = 正在读取软件包…

# Tabs
tab-details = 详情
tab-dependencies = 依赖
tab-files = 文件
toggle-sidebar = 显示或隐藏侧边栏

# Installed state
state-not-installed = 未安装
state-installed = 已安装版本 { $version }
state-upgrade = 已安装版本 { $version }，可以升级
state-downgrade = 已安装的版本 { $version } 比此文件更新
state-unknown = 安装状态未知

# Actions
action-install = 安装
action-reinstall = 重新安装
action-upgrade = 升级
action-downgrade = 降级
action-remove = 卸载
cancel = 取消
dismiss = 关闭
retry = 重试

# Progress
progress-installing = 正在安装
progress-reinstalling = 正在重新安装
progress-upgrading = 正在升级
progress-downgrading = 正在降级
progress-removing = 正在卸载
operation-complete = 已成功完成
operation-failed = 操作未能完成

# Metadata
meta-section-package = 软件包
meta-section-other = 其他
meta-package = 名称
meta-version = 版本
meta-format = 格式
meta-architecture = 架构
meta-maintainer = 维护者
meta-section = 分类
meta-license = 许可证
meta-homepage = 网站
meta-installed-size = 安装后大小
meta-file-size = 文件大小
meta-path = 文件
meta-description = 描述

# Dependency kinds
dep-pre-depends = 安装前必需
dep-depends = 必需
dep-recommends = 推荐
dep-suggests = 建议
dep-conflicts = 冲突于
dep-breaks = 破坏
dep-replaces = 替换
dep-provides = 提供

# Dependency status
dep-status-installed = 已安装 ({ $version })
dep-status-available = 可安装 ({ $version })
dep-status-provided-by = 由 { $providers } 提供
dep-status-missing = 不可用
dep-status-unknown = 未检查
dep-alternatives = 或
dependencies-none = 此软件包未声明任何依赖。
dependencies-resolving = 正在对照您的系统检查依赖…
dependencies-unsatisfiable = { $count ->
   *[other] 有 { $count } 项必需依赖无法满足。
  }

# Install plan
plan-title = 将会发生什么
plan-install = 安装
plan-upgrade = 升级
plan-downgrade = 降级
plan-remove = 移除
plan-resolving = 正在计算将要安装的内容…
plan-no-changes = 无需任何更改。
plan-additional = { $count ->
   *[other] 将额外安装 { $count } 个软件包。
  }
plan-download = 需下载 { $size }
plan-disk = 占用磁盘空间 { $size }
plan-blocked = 此软件包在当前状态下无法安装：

# Files
files-none = 此软件包不安装任何文件。
files-hidden = 文件列表已在设置中关闭。
files-count = { $count ->
   *[other] { $count } 个文件
  }
files-truncated = 显示 { $total } 个条目中的前 { $shown } 个。
files-link = 链接到 { $target }

# Supported formats
supported-formats = 支持的格式
format-deb = Debian 软件包
format-rpm = RPM 软件包
format-flatpak = Flatpak
format-appimage = AppImage
format-ready = 已支持
format-missing = 需要 { $tools }

# Settings
settings-appearance = 外观
settings-theme = 主题
theme-dark = 深色
theme-light = 浅色
theme-system = 跟随桌面
settings-behaviour = 行为
settings-privilege-backend = 安装方式
privilege-auto = PackageKit 或系统工具
privilege-packagekit = 仅 PackageKit
privilege-native = 仅系统工具
settings-show-recommends = 显示推荐的软件包
settings-show-suggests = 显示建议的软件包
settings-show-file-list = 显示文件列表

# Confirmation
confirm-remove-title = 卸载 { $name }？
confirm-remove-body = 这将从您的系统中移除该软件包。

# About
about-repository = 代码仓库
about-support = 报告问题

# Errors
error-unknown-format = { $file } 不是此应用能够打开的软件包文件。
error-unsupported-format = { $format } 文件需要 { $tools }，但它尚未安装。
error-missing-tool = { $program } 尚未安装。
error-command-failed = { $program } 报告了一个问题：{ $detail }
error-timeout = { $program } 运行时间过长，已被停止。
error-parse = 无法解析该软件包：{ $detail }
error-packagekit = PackageKit 报告了一个问题：{ $detail }
error-not-authorized = 身份验证被取消或拒绝。
error-not-implemented = 对 { $format } 文件的支持尚未实现。
