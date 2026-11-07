#!/bin/sh
# Runs the debug daemon against the dev config, forwarding every argument
# untouched (quoted, so a JSON request for `api` stays one argument).
root=$(cd "$(dirname "$0")/.." && pwd)
exec "$root/target/debug/cohesiond" -c "$root/dev" "$@"
