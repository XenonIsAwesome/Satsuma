#!/bin/sh
# Satsuma bootstrap installer.
#
# Hosted (via .github/workflows/docs-deploy.yml, which copies this file
# verbatim into the published docs/ Pages site - see "Assemble site/" in
# that workflow) at:
#
#   https://xenonisawesome.github.io/Satsuma/install.sh
#
# Usage:
#   curl -fsSL https://xenonisawesome.github.io/Satsuma/install.sh | sh
#   curl -fsSL https://xenonisawesome.github.io/Satsuma/install.sh | sh -s -- v1.2.0
#   curl -fsSL https://xenonisawesome.github.io/Satsuma/install.sh | sh -s -- 1.2.0
#
# With no version argument, installs the latest published GitHub Release.
# Only Linux is supported here - detects the distro's package manager
# (dpkg/rpm, falling back to /etc/os-release's ID_LIKE if neither binary
# is present) to pick .deb vs .rpm vs a portable .AppImage, then downloads
# and installs the matching asset .github/workflows/deploy.yml built and
# attached to that release. Windows users: see install.ps1 alongside this
# script - `irm https://xenonisawesome.github.io/Satsuma/install.ps1 | iex`.
#
# POSIX sh, deliberately - the interpreter running a piped `sh` script
# depends on what's already on the target machine (often dash, not bash),
# so this avoids bash-only syntax (arrays, [[, etc.) throughout.

set -eu

REPO="XenonIsAwesome/Satsuma"
API_BASE="https://api.github.com/repos/$REPO"
RELEASES_URL="https://github.com/$REPO/releases"

VERSION="${1:-latest}"

log() {
  printf '==> %s\n' "$1"
}

die() {
  printf 'error: %s\n' "$1" >&2
  exit 1
}

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || die "this script needs '$1', which isn't on PATH"
}

need_cmd curl
need_cmd grep
need_cmd sed

# --- 1. OS check ------------------------------------------------------------

os="$(uname -s)"
case "$os" in
  Linux) ;;
  Darwin)
    die "Satsuma doesn't ship for macOS yet - see $RELEASES_URL"
    ;;
  *)
    die "this bootstrap script only supports Linux. Windows users: irm https://xenonisawesome.github.io/Satsuma/install.ps1 | iex"
    ;;
esac

# --- 2. Pick an installer format for this distro ----------------------------

# Trust the package-manager binary over /etc/os-release's ID - it's the
# direct answer to "can this machine actually consume a .deb/.rpm", and
# avoids maintaining a distro-name-to-format map that inevitably misses a
# derivative distro. /etc/os-release's ID_LIKE is only consulted as a
# fallback, for minimal/container images that have neither tool installed
# yet (dropping straight to the AppImage fallback in that case would
# otherwise ignore a perfectly good system package manager).
format=""
distro_name="$os"

if command -v dpkg >/dev/null 2>&1; then
  format="deb"
elif command -v rpm >/dev/null 2>&1; then
  format="rpm"
elif [ -r /etc/os-release ]; then
  # shellcheck disable=SC1091
  . /etc/os-release
  distro_name="${PRETTY_NAME:-$os}"
  case " ${ID:-} ${ID_LIKE:-} " in
    *" debian "*|*" ubuntu "*) format="deb" ;;
    *" fedora "*|*" rhel "*|*" suse "*) format="rpm" ;;
  esac
fi

if [ -z "$format" ]; then
  format="appimage"
  log "Couldn't detect a supported package manager (dpkg/rpm) on $distro_name - falling back to the portable AppImage."
else
  log "Detected $distro_name -> installing the .$format package."
fi

# --- 3. Resolve the release and find the matching asset ---------------------

case "$VERSION" in
  latest) api_url="$API_BASE/releases/latest" ;;
  v*) api_url="$API_BASE/releases/tags/$VERSION" ;;
  *) api_url="$API_BASE/releases/tags/v$VERSION" ;;
esac

log "Looking up the $VERSION release..."
release_json="$(curl -fsSL "$api_url")" || die "couldn't find a release for '$VERSION' at $api_url (check the version exists: $RELEASES_URL)"

case "$format" in
  deb) pattern='"browser_download_url": *"[^"]*\.deb"' ;;
  rpm) pattern='"browser_download_url": *"[^"]*\.rpm"' ;;
  appimage) pattern='"browser_download_url": *"[^"]*\.AppImage"' ;;
esac

asset_url="$(printf '%s\n' "$release_json" | grep -o "$pattern" | head -n1 | sed -E 's/^"browser_download_url": *"(.*)"$/\1/')"
[ -n "$asset_url" ] || die "the $VERSION release has no .$format asset - it may still be building, or this platform isn't supported by that release yet"

# --- 4. Download -------------------------------------------------------------

tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT

asset_file="$tmp_dir/$(basename "$asset_url")"
log "Downloading $(basename "$asset_url")..."
curl -fsSL -o "$asset_file" "$asset_url" || die "download failed: $asset_url"
[ -s "$asset_file" ] || die "downloaded file is empty: $asset_file"

# --- 5. Install ---------------------------------------------------------------

if [ "$(id -u)" = "0" ]; then
  sudo=""
else
  need_cmd sudo
  sudo="sudo"
fi

case "$format" in
  deb)
    log "Installing with dpkg..."
    if ! $sudo dpkg -i "$asset_file"; then
      log "dpkg reported missing dependencies - resolving with apt-get..."
      $sudo apt-get install -f -y
    fi
    ;;
  rpm)
    log "Installing with the system's rpm-based package manager..."
    if command -v dnf >/dev/null 2>&1; then
      $sudo dnf install -y "$asset_file"
    elif command -v zypper >/dev/null 2>&1; then
      $sudo zypper --non-interactive install "$asset_file"
    elif command -v yum >/dev/null 2>&1; then
      $sudo yum install -y "$asset_file"
    else
      $sudo rpm -i "$asset_file"
    fi
    ;;
  appimage)
    install_dir="$HOME/.local/bin"
    mkdir -p "$install_dir"
    dest="$install_dir/satsuma"
    cp "$asset_file" "$dest"
    chmod +x "$dest"
    log "Installed to $dest"
    case ":$PATH:" in
      *":$install_dir:"*) ;;
      *) log "$install_dir isn't on your PATH - add it (e.g. in ~/.profile) or run $dest directly." ;;
    esac
    ;;
esac

log "Done. Launch Satsuma from your application menu, or run 'satsuma' if it's on your PATH."
