# Pele

The Mermaid renderer the daemon draws diagrams with (`cohesion-js/src/mermaid.rs`),
the only copy of it in this repo: clients ask the daemon, and a Rust build never
needs `web/node_modules`.

- Package: [`pele`](https://github.com/obsidianmd/pele) on npm, MIT (`LICENSE`)
- Version: 1.13.0
- File: `dist/pele.global.js`, unmodified
- SHA-256: `0b0a46c57a5a6971670c09f5ec4066705bd78505f90fd11adf6260325efa0910`

A test keeps the version here equal to the banner of the file.

To change version: `npm pack pele@<version>` somewhere outside the repo, copy
`package/dist/pele.global.js` and `package/LICENSE` here, and update this file.
