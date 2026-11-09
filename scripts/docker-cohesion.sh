#!/bin/sh
# The container image's `cohesion`: the real CLI, which on first use connects
# itself to the daemon in the same container through its socket. The identity
# it makes is kept on the volume, so it stays one client across containers.
cli=/usr/local/libexec/cohesion
if [ ! -e "$XDG_CONFIG_HOME/cohesion/config.json" ]; then
	"$cli" setup --local --name "Container (CLI)" >/dev/null || exit
fi
exec "$cli" "$@"
