#!/bin/sh
# Releases the checked-out commit: works out its version, vXXXYYYYY (XXX is
# MAJOR_VERSION, YYYYY is one more than the last local tag of that major
# version, or 0 for its first), tags the commit with it, pushes the tag and
# creates a GitHub release listing the commits since the last one (or, for the
# first, since MAJOR_VERSION changed). It reads only the local repository. The
# Release workflow runs it; pushing the tag from anywhere else starts the image
# build too. Takes no arguments.
set -e
cd "$(dirname "$0")/.."
repo=https://github.com/cohesionhq/cohesion

commit=$(git rev-parse HEAD)

major=$(git show "$commit:MAJOR_VERSION" | tr -d '[:space:]')
case $major in
[0-9][0-9][0-9]) ;;
*) echo "MAJOR_VERSION must be three digits, not '$major'" >&2; exit 1 ;;
esac
pattern="v$major[0-9][0-9][0-9][0-9][0-9]"
released=$(git tag --points-at "$commit" --list "$pattern")
if [ -n "$released" ]; then
	echo "Nothing to release: this commit is already $released." >&2
	exit 1
fi

last=$(git tag --list "$pattern" --sort=-refname | head -n 1)
if [ -n "$last" ]; then
	# expr reads the digits as decimal, where $(( )) takes a leading 0 for octal.
	minor=$(expr "${last#"v$major"}" + 1)
else
	minor=0
fi
if [ "$minor" -gt 99999 ]; then
	echo "Minor version $minor doesn't fit in a version's five digits" >&2
	exit 1
fi
version=v$major$(printf %05d "$minor")

notes=$(mktemp)
trap 'rm -f "$notes"' EXIT
previous=$(git describe --tags --abbrev=0 --match 'v*' "$commit" 2>/dev/null || true)
if [ -n "$previous" ]; then
	git log --format='- %s (%h)' "$previous..$commit" >"$notes"
	printf '\n**Full Changelog**: %s/compare/%s...%s\n' "$repo" "$previous" "$version" >>"$notes"
else
	# The first release lists what this major version has added.
	base=$(git log -1 --format=%H "$commit" -- MAJOR_VERSION)
	git log --format='- %s (%h)' "$base..$commit" >"$notes"
fi

git tag -a "$version" -m "$version" "$commit"
git push --quiet origin "refs/tags/$version"
gh release create "$version" --verify-tag --title "$version" --notes-file "$notes"

if [ -n "$GITHUB_OUTPUT" ]; then
	echo "version=$version" >>"$GITHUB_OUTPUT"
fi
echo "Released $version. The image is ghcr.io/cohesionhq/cohesion:$version once it's built."
