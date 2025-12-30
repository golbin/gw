#!/usr/bin/env bash
set -euo pipefail

VERSION=""
BIN_DIR="$HOME/.local/bin"

usage() {
  cat <<USAGE
Usage: install.sh [--version vX.Y.Z] [--bin-dir DIR]

Options:
  --version   Install a specific version (default: latest)
  --bin-dir   Install directory (default: $HOME/.local/bin)
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --version)
      VERSION="$2"
      shift 2
      ;;
    --bin-dir)
      BIN_DIR="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      usage
      exit 1
      ;;
  esac
done

if [[ -z "$VERSION" ]]; then
  VERSION=$(curl -fsSL https://api.github.com/repos/golbin/gw/releases/latest | sed -n 's/.*"tag_name": "\([^"]*\)".*/\1/p')
fi

if [[ -z "$VERSION" ]]; then
  echo "Failed to resolve version" >&2
  exit 1
fi

OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

case "$OS" in
  darwin)
    OS="apple-darwin"
    ;;
  linux)
    OS="unknown-linux-gnu"
    ;;
  *)
    echo "Unsupported OS: $OS" >&2
    exit 1
    ;;
esac

case "$ARCH" in
  x86_64|amd64)
    ARCH="x86_64"
    ;;
  arm64|aarch64)
    ARCH="aarch64"
    ;;
  *)
    echo "Unsupported ARCH: $ARCH" >&2
    exit 1
    ;;
esac

TARGET="$ARCH-$OS"
ARCHIVE="gw-${VERSION}-${TARGET}.tar.gz"
BASE_URL="https://github.com/golbin/gw/releases/download/${VERSION}"

TMP_DIR=$(mktemp -d)
trap 'rm -rf "$TMP_DIR"' EXIT

curl -fL "${BASE_URL}/${ARCHIVE}" -o "$TMP_DIR/$ARCHIVE"
curl -fL "${BASE_URL}/SHA256SUMS" -o "$TMP_DIR/SHA256SUMS"

pushd "$TMP_DIR" >/dev/null
if command -v sha256sum >/dev/null 2>&1; then
  sha256sum -c SHA256SUMS --ignore-missing
elif command -v shasum >/dev/null 2>&1; then
  shasum -a 256 -c SHA256SUMS
else
  echo "No SHA256 checker found" >&2
  exit 1
fi

mkdir -p "$BIN_DIR"
tar -xzf "$ARCHIVE"
install -m 0755 gw "$BIN_DIR/gw"

popd >/dev/null

echo "Installed gw to $BIN_DIR/gw"
