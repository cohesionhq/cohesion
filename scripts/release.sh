#!/bin/sh
# Releases what's on GitHub's master: works out its version, vXXXYYYYY (XXX is
# MAJOR_VERSION, YYYYY counts the commits since it last changed), tags the
# commit with it, pushes the tag and creates a GitHub release listing the
# commits since the last one (or, for the first, since MAJOR_VERSION changed). The Release workflow runs it; pushing the tag
# from anywhere else starts the image build too. Takes no arguments.
set -e
cd "$(dirname "$0")/.."
repo=https://github.com/cohesionhq/cohesion

git fetch --quiet --tags origin master
commit=$(git rev-parse origin/master)

major=$(git show "$commit:MAJOR_VERSION" | tr -d '[:space:]')
case $major in
[0-9][0-9][0-9]) ;;
*) echo "MAJOR_VERSION must be three digits, not '$major'" >&2; exit 1 ;;
esac
base=$(git log -1 --format=%H "$commit" -- MAJOR_VERSION)
minor=$(git rev-list --count "$base..$commit")
if [ "$minor" -gt 99999 ]; then
	echo "Minor version $minor doesn't fit in a version's five digits" >&2
	exit 1
fi
version=v$major$(printf %05d "$minor")

if git rev-parse --quiet --verify "refs/tags/$version" >/dev/null; then
	echo "Nothing to release: master is already $version." >&2
	exit 1
fi

notes=$(mktemp)
trap 'rm -f "$notes"' EXIT
previous=$(git describe --tags --abbrev=0 --match 'v*' "$commit" 2>/dev/null || true)
if [ -n "$previous" ]; then
	git log --format='- %s (%h)' "$previous..$commit" >"$notes"
	printf '\n**Full Changelog**: %s/compare/%s...%s\n' "$repo" "$previous" "$version" >>"$notes"
else
	# The first release lists what this major version has added.
	git log --format='- %s (%h)' "$base..$commit" >"$notes"
fi

git tag -a "$version" -m "$version" "$commit"
git push --quiet origin "refs/tags/$version"
gh release create "$version" --verify-tag --title "$version" --notes-file "$notes"

if [ -n "$GITHUB_OUTPUT" ]; then
	echo "version=$version" >>"$GITHUB_OUTPUT"
fi
echo "Released $version. The image is ghcr.io/cohesionhq/cohesion:$version once it's built."
