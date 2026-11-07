#!/bin/sh
set -e
cd "$(dirname "$0")/.."

# cohesiond embeds the web UI, so build it first.
(cd web && npm ci && npm run build)
cargo install --path cohesiond
