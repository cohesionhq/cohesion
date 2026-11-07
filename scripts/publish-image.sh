#!/bin/sh
# Builds the image from a clean copy of a commit (default HEAD), so nothing
# uncommitted gets in, tags it by commit and pushes it to GHCR. Prints the
# reference to pull by digest. Signs in with `gh auth token`, which needs the
# write:packages scope, and leaves no credential behind.
set -e
cd "$(dirname "$0")/.."
image=ghcr.io/cohesionhq/cohesion
commit=$(git rev-parse --verify "${1:-HEAD}^{commit}")
tag=$image:$(git rev-parse --short "$commit")

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mkdir "$work/context"
git archive "$commit" | tar -x -C "$work/context"

podman build --label "org.opencontainers.image.revision=$commit" \
	--label org.opencontainers.image.source=https://github.com/cohesionhq/cohesion \
	-t "$tag" "$work/context"

gh auth token | podman login --authfile "$work/auth.json" -u "$(gh api user --jq .login)" --password-stdin ghcr.io
podman push --authfile "$work/auth.json" --digestfile "$work/digest" "$tag"
echo "$image@$(cat "$work/digest")"
