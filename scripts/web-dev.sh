#!/bin/sh
# Runs the web UI's live-reload loop against the dev client config: the debug CLI's
# `open --dev` proxy and Vite, sharing a random token made fresh for this run and
# passed in the environment (never on a command line). Extra arguments go to Vite.
# The proxy listens on $VITE_PROXY_PORT, 4000 by default.
COHESION_DEV_TOKEN=$(head -c 24 /dev/urandom | od -An -tx1 | tr -d ' \n')
export COHESION_DEV_TOKEN
port=${VITE_PROXY_PORT:-4000}
root=$(cd "$(dirname "$0")/.." && pwd)

"$root/target/debug/cohesion" -c "$root/dev/client" open --dev --port "$port" --no-browser &
proxy=$!
trap 'kill $proxy 2>/dev/null' EXIT INT TERM

cd "$root/web" && npm run dev -- "$@"
