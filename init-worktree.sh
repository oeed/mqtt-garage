#!/usr/bin/env bash
#
# Prepare a fresh git worktree to build mqtt-garage.
#
# A new worktree would otherwise bootstrap its own ESP-IDF toolchain under .embuild
# (a slow, multi-GB clone) and is missing the gitignored garage-config.*.toml files.
# On top of that, a from-scratch ESP-IDF v5.2.3 bootstrap on this machine lands in a
# broken state (Python 3.9's importlib.metadata can't resolve dotted dependency names
# like "ruamel.yaml", and the shallow clone leaves several submodules empty).
#
# Instead of re-bootstrapping, this script points the worktree at the *main* checkout's
# already-provisioned toolchain and config. Run it once from inside a new worktree:
#
#     ./init-worktree.sh
#
# The main checkout must have been built successfully at least once (so its .embuild is
# fully provisioned). If a fresh main .embuild ever needs the same repairs, see
# README/notes: patch tools/check_python_dependencies.py to normalise names, and run
# `git submodule update --init --recursive` (fetching pinned commits) in the IDF dir.

set -euo pipefail

# The main worktree is always the first entry in `git worktree list`.
main_root="$(git worktree list --porcelain | awk '/^worktree /{print $2; exit}')"
this_root="$(git rev-parse --show-toplevel)"

if [ "$main_root" = "$this_root" ]; then
  echo "This is the main checkout — nothing to set up."
  exit 0
fi

echo "Main checkout : $main_root"
echo "This worktree : $this_root"

# Share the main checkout's ESP-IDF toolchain rather than bootstrapping a new one.
if [ -d "$main_root/.embuild" ]; then
  rm -rf "$this_root/.embuild"
  ln -s "$main_root/.embuild" "$this_root/.embuild"
  echo "linked .embuild -> $main_root/.embuild"
else
  echo "WARNING: $main_root/.embuild not found — build the main checkout once first." >&2
fi

# Link the gitignored build-time config files.
for f in garage-config.debug.toml garage-config.release.toml; do
  if [ -e "$main_root/$f" ]; then
    ln -sf "$main_root/$f" "$this_root/$f"
    echo "linked $f"
  else
    echo "note: $main_root/$f not present (skipped)"
  fi
done

echo "Done. Build with: cargo build --release"
echo "(Avoid building the main checkout and a worktree at the same time — they share .embuild.)"
