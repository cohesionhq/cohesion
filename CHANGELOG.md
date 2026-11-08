# Changelog

What changes in each release of Cohesion, newest first. A release is
`vXXXYYYYY`, the version `cohesion status` shows and the container image's tag
(see "Publish an image" in [CONTRIBUTING.md](CONTRIBUTING.md)). Add an entry
under "Unreleased" with any change users would notice.

## Unreleased

- Cohesion is open source, under the GNU Affero General Public License, version
  3 only. See [Licensing](docs/licensing.md).
- The repository starts a new history at `cohesionhq/cohesion`, with a code of
  conduct, a security policy, issue and pull request templates, and checks that
  run on every pull request.
- `THIRD_PARTY_NOTICES.md` lists the crates in `cohesiond` and `cohesion`, and
  the web UI's build lists the packages it bundles in `third-party-licenses.md`.
- Extensions install from a URL: under Extensions, Install from URL downloads
  the script and opens the same review as a file. A download stops at 5 MB.
- `cohesiond export <file>` copies a server's data, images included, while it
  runs, and `cohesiond import <file>` replaces another, stopped server's data
  with it. The importing server keeps its own key, so every device pairs with
  it again.
- `rust-toolchain.toml` pins the Rust release Cohesion builds with.
- The web UI's second design pass: the Dashboard's greeting leads the page, and a
  task due today can be completed from it; documents read as rendered Markdown
  away from the line being edited, in a prose-width column; Documents rows show
  where their text starts; an actor session's header leads with its status; a
  disabled button turns grey; a working session's mark is grey, so the accent
  means unread.
