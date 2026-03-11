#!/usr/bin/env bash
# Usage: ./scripts/release.sh v0.1.3
#
# Bumps version in Cargo.toml, commits, tags, and optionally pushes.
# Prevents the "forgot to update Cargo.toml" problem.

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BOLD='\033[1m'
RESET='\033[0m'

die() { echo -e "${RED}error:${RESET} $1" >&2; exit 1; }
info() { echo -e "${GREEN}=>${RESET} $1"; }
warn() { echo -e "${YELLOW}warn:${RESET} $1"; }

# ── Validate input ──────────────────────────────────────────────────────────

TAG="${1:-}"
[ -z "$TAG" ] && die "Usage: ./scripts/release.sh v0.1.3"

# Accept both v0.1.3 and 0.1.3
VERSION="${TAG#v}"

# Validate semver format
if ! echo "$VERSION" | grep -qE '^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.]+)?$'; then
    die "Invalid version format: $VERSION (expected semver like 0.1.3 or 0.1.3-rc1)"
fi

# ── Pre-flight checks ──────────────────────────────────────────────────────

# Must be in repo root
[ -f Cargo.toml ] || die "Run from the repository root (where Cargo.toml is)"

# Working tree must be clean
if [ -n "$(git status --porcelain)" ]; then
    die "Working tree is dirty. Commit or stash changes first."
fi

# Tag must not already exist
if git rev-parse "v$VERSION" >/dev/null 2>&1; then
    die "Tag v$VERSION already exists"
fi

CURRENT_VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/')
info "Current version: ${BOLD}$CURRENT_VERSION${RESET}"
info "New version:     ${BOLD}$VERSION${RESET}"

# ── Bump version ────────────────────────────────────────────────────────────

# Update Cargo.toml (first version = line in [package])
# Use awk to replace only the first occurrence (works on macOS and Linux)
awk -v old="$CURRENT_VERSION" -v new="$VERSION" '
    done == 0 && /^version = "/ { sub(old, new); done=1 }
    { print }
' Cargo.toml > Cargo.toml.tmp && mv Cargo.toml.tmp Cargo.toml
info "Updated Cargo.toml"

# Update Cargo.lock
cargo check --quiet 2>/dev/null
info "Updated Cargo.lock"

# ── Quality gate ────────────────────────────────────────────────────────────

info "Running quality checks..."
cargo fmt --all --check || die "cargo fmt failed — run 'cargo fmt --all' first"
cargo clippy --all-targets --quiet 2>/dev/null || die "cargo clippy failed"
cargo test --quiet 2>/dev/null || die "cargo test failed"
info "All checks passed"

# ── Commit & tag ────────────────────────────────────────────────────────────

git add Cargo.toml Cargo.lock
git commit -m "chore: bump version to v$VERSION"
git tag -a "v$VERSION" -m "Release v$VERSION"

info "Created commit and tag ${BOLD}v$VERSION${RESET}"

# ── Push prompt ─────────────────────────────────────────────────────────────

echo ""
echo -e "${BOLD}Ready to push. Run:${RESET}"
echo ""
echo "  git push origin main --tags"
echo ""
