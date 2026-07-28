#!/usr/bin/env bash
#
# Build, install and package cosmic-package-installer.
#
# One script so that CI and a local build take exactly the same path — a
# release that is assembled differently from the way it was tested is a release
# that has not been tested.
#
#   ./install.sh build       release build
#   ./install.sh install     build, then install system-wide (needs root)
#   ./install.sh uninstall   remove an installed copy (needs root)
#   ./install.sh deb         build a .deb into dist/
#   ./install.sh rpm         build an .rpm into dist/
#   ./install.sh tarball     build a portable tarball into dist/
#   ./install.sh all         deb + rpm + tarball
#   ./install.sh check       cargo check, clippy and tests, plus locale checks
#   ./install.sh hooks       point git at the repository's hooks
#
set -euo pipefail

APP_NAME="cosmic-package-installer"
APP_ID="com.github.cosmic_package_installer"

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT_DIR"

DIST_DIR="$ROOT_DIR/dist"
BIN="$ROOT_DIR/target/release/$APP_NAME"

PREFIX="${PREFIX:-/usr}"

# Every packaging target passes this. It forces the diagnostic-logging switch
# in src/debug.rs off at compile time, so a release cannot ship with logging
# left on regardless of what the developer switch says. Tools that run their
# own cargo invocation need it threaded through explicitly.
RELEASE_FEATURES="release-build"

info()  { printf '\033[1;34m==>\033[0m %s\n' "$*"; }
warn()  { printf '\033[1;33m warning:\033[0m %s\n' "$*" >&2; }
die()   { printf '\033[1;31m error:\033[0m %s\n' "$*" >&2; exit 1; }

need() {
    command -v "$1" >/dev/null 2>&1 || die "$1 is required but not installed${2:+ ($2)}"
}

version() {
    # Read from the manifest so the version has a single source of truth.
    grep -m1 '^version' Cargo.toml | cut -d'"' -f2
}

# ── Build ───────────────────────────────────────────────────────────────────

do_build() {
    need cargo
    info "Building $APP_NAME $(version) (release, features: $RELEASE_FEATURES)"
    cargo build --release --features "$RELEASE_FEATURES"
    verify_logging_stripped
}

# Confirm the release feature actually removed the logging code, rather than
# trusting that it did. The log path is a string literal in the binary when
# logging is compiled in and absent when it is not, which makes this a direct
# check on the built artefact.
verify_logging_stripped() {
    local log_path
    log_path="$(grep -m1 'pub const PATH' src/debug.rs | cut -d'"' -f2)"
    [ -n "$log_path" ] || { warn "could not determine the debug log path; skipping check"; return; }

    if command -v strings >/dev/null 2>&1; then
        if strings "$BIN" | grep -qF "$log_path"; then
            die "the debug log path '$log_path' is present in the release binary — diagnostic logging was not compiled out"
        fi
        info "Verified: diagnostic logging is compiled out"
    else
        warn "'strings' not available; could not verify that logging was compiled out"
    fi
}

# ── Checks ──────────────────────────────────────────────────────────────────

do_check() {
    need cargo
    info "cargo check"
    cargo check --all-targets
    info "cargo clippy"
    cargo clippy --all-targets
    info "cargo test"
    cargo test
    info "locale consistency"
    need python3
    python3 tools/check-locales.py
}

# ── Install / uninstall ─────────────────────────────────────────────────────

install_paths() {
    # Printed by uninstall too, so the two cannot drift apart.
    cat <<EOF
$PREFIX/bin/$APP_NAME
$PREFIX/share/applications/$APP_ID.desktop
$PREFIX/share/metainfo/$APP_ID.metainfo.xml
$PREFIX/share/icons/hicolor/scalable/apps/$APP_ID.svg
EOF
}

do_install() {
    [ -f "$BIN" ] || do_build
    [ "$(id -u)" -eq 0 ] || die "install needs root; re-run with sudo"

    info "Installing to $PREFIX"
    install -Dm755 "$BIN" "$PREFIX/bin/$APP_NAME"
    install -Dm644 resources/app.desktop "$PREFIX/share/applications/$APP_ID.desktop"
    install -Dm644 resources/app.metainfo.xml "$PREFIX/share/metainfo/$APP_ID.metainfo.xml"
    install -Dm644 resources/icon.svg "$PREFIX/share/icons/hicolor/scalable/apps/$APP_ID.svg"

    refresh_desktop_databases
    info "Installed $APP_NAME $(version)"
}

do_uninstall() {
    [ "$(id -u)" -eq 0 ] || die "uninstall needs root; re-run with sudo"

    info "Removing $APP_NAME"
    while IFS= read -r path; do
        [ -e "$path" ] && rm -f "$path" && echo "  removed $path"
    done < <(install_paths)

    refresh_desktop_databases
    info "Removed $APP_NAME"
}

# The desktop entry declares MIME types, so without this the application does
# not appear as a handler for package files until the next login.
refresh_desktop_databases() {
    command -v update-desktop-database >/dev/null 2>&1 \
        && update-desktop-database -q "$PREFIX/share/applications" || true
    command -v gtk-update-icon-cache >/dev/null 2>&1 \
        && gtk-update-icon-cache -qtf "$PREFIX/share/icons/hicolor" 2>/dev/null || true
}

# ── Git hooks ───────────────────────────────────────────────────────────────

# Hooks live in the repository so they are versioned with the code, which means
# git has to be pointed at them once per clone.
do_hooks() {
    need git
    git rev-parse --git-dir >/dev/null 2>&1 || die "not inside a git repository"
    git config core.hooksPath .githooks
    info "git hooks enabled (core.hooksPath = .githooks)"
    info "post-commit bumps the version on feat: and fix: commits"
}

# ── Packaging ───────────────────────────────────────────────────────────────

do_deb() {
    need cargo
    command -v cargo-deb >/dev/null 2>&1 || die "cargo-deb is required (cargo install cargo-deb)"
    mkdir -p "$DIST_DIR"

    info "Building .deb"
    # `depends = "$auto"` resolves shared-library dependencies with
    # dpkg-shlibdeps, which inspects the binary and maps each library it needs
    # back to the package owning it. A developer's LD_LIBRARY_PATH — a Flatpak
    # runtime, a local build of a library — makes it resolve to a path no
    # package owns, at which point it gives up and cargo-deb produces a package
    # declaring no dependencies at all. That package installs fine and then
    # fails to start. Packaging therefore runs with the variable cleared.
    env -u LD_LIBRARY_PATH cargo deb --output "$DIST_DIR" -- --features "$RELEASE_FEATURES"

    # cargo-deb's build lands in target/release, so the artefact is available
    # to check even though this did not go through do_build.
    verify_logging_stripped

    local package
    package="$(ls -1t "$DIST_DIR"/*.deb | head -1)"
    verify_deb_dependencies "$package"
    info "Wrote $package"
}

# Refuse to ship a .deb that declares no dependencies.
#
# This binary links libc, libgcc and libxkbcommon at minimum, so an empty
# `Depends` field never means "nothing needed" — it means dependency resolution
# failed quietly. Catching it here is the difference between a build error and
# a user reporting that the application will not start.
verify_deb_dependencies() {
    local package="$1"
    command -v dpkg-deb >/dev/null 2>&1 || { warn "dpkg-deb not available; could not verify dependencies"; return; }

    local depends
    depends="$(LC_ALL=C dpkg-deb --field "$package" Depends 2>/dev/null | tr -d '[:space:]')"
    [ -n "$depends" ] || die "$(basename "$package") declares no dependencies — dpkg-shlibdeps failed. Check that dpkg-dev is installed and that no LD_LIBRARY_PATH override is in effect."

    info "Verified: dependencies resolved ($(LC_ALL=C dpkg-deb --field "$package" Depends))"
}

do_rpm() {
    need cargo
    command -v cargo-generate-rpm >/dev/null 2>&1 \
        || die "cargo-generate-rpm is required (cargo install cargo-generate-rpm)"
    mkdir -p "$DIST_DIR"

    # cargo-generate-rpm packages an existing build rather than making its own,
    # so the release build has to happen first.
    do_build

    info "Building .rpm"
    cargo generate-rpm --output "$DIST_DIR"
    info "Wrote $(ls -1 "$DIST_DIR"/*.rpm | tail -1)"
}

do_tarball() {
    do_build
    mkdir -p "$DIST_DIR"

    local name="$APP_NAME-$(version)-$(uname -m)"
    local staging
    staging="$(mktemp -d)"

    # Cleaned up on EXIT rather than RETURN. A RETURN trap is not scoped to the
    # function that set it: it stays armed and fires again when the *caller*
    # returns, by which point `staging` is out of scope and `set -u` aborts the
    # script — after the tarball has been written, so the artefact looks fine
    # and only the exit status says otherwise.
    trap 'rm -rf "${staging:-}"' EXIT

    info "Building tarball"
    install -Dm755 "$BIN" "$staging/$name/bin/$APP_NAME"
    install -Dm644 resources/app.desktop "$staging/$name/share/applications/$APP_ID.desktop"
    install -Dm644 resources/app.metainfo.xml "$staging/$name/share/metainfo/$APP_ID.metainfo.xml"
    install -Dm644 resources/icon.svg "$staging/$name/share/icons/hicolor/scalable/apps/$APP_ID.svg"
    install -Dm644 LICENSE "$staging/$name/LICENSE"
    install -Dm644 README.md "$staging/$name/README.md"

    tar -czf "$DIST_DIR/$name.tar.gz" -C "$staging" "$name"

    rm -rf "$staging"
    trap - EXIT

    info "Wrote $DIST_DIR/$name.tar.gz"
}

do_all() {
    do_deb
    do_rpm
    do_tarball
}

# ── Entry point ─────────────────────────────────────────────────────────────

case "${1:-build}" in
    build)     do_build ;;
    check)     do_check ;;
    install)   do_install ;;
    uninstall) do_uninstall ;;
    deb)       do_deb ;;
    rpm)       do_rpm ;;
    tarball)   do_tarball ;;
    all)       do_all ;;
    hooks)     do_hooks ;;
    *)
        sed -n '3,18p' "${BASH_SOURCE[0]}" | sed 's/^# \?//'
        exit 1
        ;;
esac
