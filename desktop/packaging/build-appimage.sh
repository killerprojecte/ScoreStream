#!/usr/bin/env bash
# Production route: use Tauri's maintained AppImage bundler on a provisioned builder.
set -euo pipefail
repo=$(cd "$(dirname "$0")/../.." && pwd)
cd "$repo/desktop"
npm ci
npm test -- --maxWorkers=1
npm run desktop:build -- --bundles appimage
