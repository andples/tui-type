#!/usr/bin/env bash
# Bump ttyp's version, tag + release it on GitHub, and update the
# homebrew-ttyp tap formula to match.
#
# Usage:
#   scripts/release.sh minor   # 0.1.0 -> 0.2.0
#   scripts/release.sh major   # 0.1.0 -> 1.0.0
#
# Requires: cargo, git, gh (authenticated), sha256sum, curl.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TAP_DIR="${TAP_DIR:-$ROOT/../homebrew-ttyp}"
CARGO_TOML="$ROOT/Cargo.toml"
FORMULA="$TAP_DIR/Formula/ttyp.rb"
GITHUB_REPO="andples/tui-type"

usage() {
    echo "Usage: $0 <minor|major>" >&2
    echo "  minor   bump X.Y.Z -> X.(Y+1).0   (the \".1\" bump)" >&2
    echo "  major   bump X.Y.Z -> (X+1).0.0   (the \"1.0\" bump)" >&2
    exit 1
}

[ $# -eq 1 ] || usage
BUMP="$1"
case "$BUMP" in
    minor|major) ;;
    *) usage ;;
esac

command -v gh >/dev/null || { echo "error: gh CLI is required" >&2; exit 1; }

if [ -n "$(git -C "$ROOT" status --porcelain)" ]; then
    echo "error: $ROOT has uncommitted changes, aborting" >&2
    exit 1
fi
if [ ! -d "$TAP_DIR" ]; then
    echo "error: tap repo not found at $TAP_DIR (set TAP_DIR to override)" >&2
    exit 1
fi
if [ -n "$(git -C "$TAP_DIR" status --porcelain)" ]; then
    echo "error: $TAP_DIR has uncommitted changes, aborting" >&2
    exit 1
fi

CURRENT_VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$CARGO_TOML" | head -1)"
[ -n "$CURRENT_VERSION" ] || { echo "error: could not read version from $CARGO_TOML" >&2; exit 1; }

IFS='.' read -r MAJOR MINOR PATCH <<< "$CURRENT_VERSION"
if [ "$BUMP" = "minor" ]; then
    NEW_VERSION="$MAJOR.$((MINOR + 1)).0"
else
    NEW_VERSION="$((MAJOR + 1)).0.0"
fi
TAG="v$NEW_VERSION"

echo "==> $CURRENT_VERSION -> $NEW_VERSION ($TAG)"

echo "==> updating Cargo.toml"
sed -i "0,/^version = \".*\"/s//version = \"$NEW_VERSION\"/" "$CARGO_TOML"

echo "==> updating Cargo.lock"
(cd "$ROOT" && cargo check -q)

echo "==> running tests"
(cd "$ROOT" && cargo test -q)

echo "==> committing version bump"
git -C "$ROOT" add Cargo.toml Cargo.lock
git -C "$ROOT" commit -m "Bump version to $NEW_VERSION"

echo "==> tagging $TAG"
git -C "$ROOT" tag -a "$TAG" -m "$TAG"

echo "==> pushing branch and tag"
git -C "$ROOT" push origin HEAD
git -C "$ROOT" push origin "$TAG"

echo "==> creating GitHub release"
gh release create "$TAG" \
    --repo "$GITHUB_REPO" \
    --title "$TAG" \
    --generate-notes

TARBALL_URL="https://github.com/$GITHUB_REPO/archive/refs/tags/$TAG.tar.gz"
echo "==> downloading tarball to compute sha256"
TMP_TARBALL="$(mktemp)"
trap 'rm -f "$TMP_TARBALL"' EXIT
curl -sL "$TARBALL_URL" -o "$TMP_TARBALL"
SHA256="$(sha256sum "$TMP_TARBALL" | cut -d' ' -f1)"
echo "    sha256: $SHA256"

echo "==> updating formula at $FORMULA"
sed -i \
    -e "s|archive/refs/tags/v[0-9][0-9.]*\.tar\.gz|archive/refs/tags/$TAG.tar.gz|" \
    -e "s/^  sha256 \".*\"/  sha256 \"$SHA256\"/" \
    "$FORMULA"

echo "==> committing and pushing tap update"
git -C "$TAP_DIR" add Formula/ttyp.rb
git -C "$TAP_DIR" commit -m "ttyp $NEW_VERSION"
git -C "$TAP_DIR" push origin HEAD

echo "==> done: $TAG released and homebrew-ttyp updated"
