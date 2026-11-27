#!/usr/bin/env bash
set -euo pipefail

# Resolve and invoke the `weft` CLI:
# 1. Global `weft` on PATH
# 2. Bundled CLI inside an installed WEFT desktop app (macOS / Linux / Windows)
# 3. Prebuilt binary in the weft workspace (target/release/weft or target/debug/weft)
# 4. `cargo run --manifest-path <repo>/Cargo.toml -p weft-cli -q --`

if command -v weft >/dev/null 2>&1; then
  exec weft "$@"
fi

for candidate in \
  "/Applications/weft.app/Contents/MacOS/weft" \
  "${HOME:+$HOME/Applications/weft.app/Contents/MacOS/weft}" \
  "${HOME:+$HOME/.local/bin/weft}" \
  "${LOCALAPPDATA:+$LOCALAPPDATA/weft/weft.exe}" \
  "${PROGRAMFILES:+$PROGRAMFILES/weft/weft.exe}"
do
  if [[ -n "$candidate" && -x "$candidate" ]]; then
    exec "$candidate" "$@"
  fi
done

find_repo_root() {
  local dir="$1"
  while [[ -n "$dir" && "$dir" != "/" ]]; do
    if [[ -f "$dir/crates/weft-cli/Cargo.toml" && -f "$dir/Cargo.toml" ]]; then
      printf '%s\n' "$dir"
      return 0
    fi
    dir="$(dirname "$dir")"
  done
  return 1
}

REPO_ROOT=""
if [[ -n "${WEFT_REPO:-}" && -f "${WEFT_REPO}/crates/weft-cli/Cargo.toml" ]]; then
  REPO_ROOT="$WEFT_REPO"
elif REPO_ROOT="$(find_repo_root "$PWD")"; then
  :
else
  SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  REPO_ROOT="$(find_repo_root "$SCRIPT_DIR" || true)"
fi

if [[ -z "$REPO_ROOT" ]]; then
  echo "error: weft CLI not found on PATH, in installed WEFT app locations, or in a local weft repository." >&2
  echo "hint: install the WEFT desktop app (on macOS, run Settings -> Install 'weft' CLI in PATH), or install via 'cargo install --git https://github.com/asinkLuno/weft weft-cli'." >&2
  exit 2
fi

if [[ -x "$REPO_ROOT/target/release/weft" ]]; then
  exec "$REPO_ROOT/target/release/weft" "$@"
elif [[ -x "$REPO_ROOT/target/debug/weft" ]]; then
  exec "$REPO_ROOT/target/debug/weft" "$@"
else
  exec cargo run --manifest-path "$REPO_ROOT/Cargo.toml" -p weft-cli -q -- "$@"
fi
