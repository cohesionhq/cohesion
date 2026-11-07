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
- `rust-toolchain.toml` pins the Rust release Cohesion builds with.
