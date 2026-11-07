#!/bin/sh
# Runs the debug CLI against the dev client config, forwarding every argument
# untouched (quoted, so a JSON request for `api` stays one argument).
root=$(cd "$(dirname "$0")/.." && pwd)
exec "$root/target/debug/cohesion" -c "$root/dev/client" "$@"
