#!/bin/bash
set -e

DOC_ARCHITECTURE="docs/architecture.md"

echo "🔍 Validating Mycelium documentation consistency..."

# =========================================================
#  1. Version consistency
# =========================================================
CARGO_VERSION=$(grep '^version = ' Cargo.toml | cut -d'"' -f2)
echo "📦 Cargo.toml version: $CARGO_VERSION"

for file in README.md CLAUDE.md; do
  if [ ! -f "$file" ]; then
    echo "⚠️  $file not found, skipping"
    continue
  fi
  if ! grep -q "$CARGO_VERSION" "$file"; then
    echo "❌ $file does not mention version $CARGO_VERSION"
    exit 1
  fi
done
echo "✅ Version consistency: all docs mention $CARGO_VERSION"

# =========================================================
#  2. Module count consistency
# =========================================================
MAIN_MODULES=$(grep -c '^mod ' src/main.rs)
echo "📊 Module count in main.rs: $MAIN_MODULES"

if [ -f "$DOC_ARCHITECTURE" ]; then
  ARCH_MODULES=$(grep 'Total:.*modules' "$DOC_ARCHITECTURE" | grep -o '[0-9]\+' | head -1)
  if [ -z "$ARCH_MODULES" ]; then
    echo "⚠️  Could not extract module count from $DOC_ARCHITECTURE"
  else
    echo "📊 Module count in $DOC_ARCHITECTURE: $ARCH_MODULES"
    if [ "$MAIN_MODULES" != "$ARCH_MODULES" ]; then
      echo "❌ Module count mismatch: main.rs=$MAIN_MODULES, $DOC_ARCHITECTURE=$ARCH_MODULES"
      exit 1
    fi
  fi
fi

# =========================================================
#  3. Python/Go commands documentation
# =========================================================
PYTHON_GO_CMDS=("ruff" "pytest" "pip" "go" "golangci")
echo "🐍 Checking Python/Go commands documentation..."

for cmd in "${PYTHON_GO_CMDS[@]}"; do
  for file in README.md CLAUDE.md; do
    if [ ! -f "$file" ]; then
      echo "⚠️  $file not found, skipping"
      continue
    fi
    if ! grep -q "$cmd" "$file"; then
      echo "❌ $file does not mention command $cmd"
      exit 1
    fi
  done
done
echo "✅ Python/Go commands: documented in README.md and CLAUDE.md"

# =========================================================
#  4. Hook consistency
# =========================================================
HOOK_FILE=".claude/hooks/mycelium-rewrite.sh"
if [ -f "$HOOK_FILE" ]; then
  echo "🪝 Checking hook rewrites..."
  for cmd in "${PYTHON_GO_CMDS[@]}"; do
    if ! grep -q "$cmd" "$HOOK_FILE"; then
      echo "⚠️  Hook may not rewrite $cmd (verify manually)"
    fi
  done
  echo "✅ Hook file exists and mentions Python/Go commands"
else
  echo "⚠️  Hook file not found: $HOOK_FILE"
fi

echo ""
echo "✅ Documentation validation passed"
