# cohesiond and the cohesion CLI in one image. See "Run in a container" in
# docs/self-hosting.md.

# cohesiond embeds the web UI's build and hands it to paired clients.
FROM docker.io/library/node:26-slim AS web
WORKDIR /src/web
COPY web/package.json web/package-lock.json ./
RUN npm ci
COPY web/ ./
RUN npm run build

# cargo-chef builds the dependencies from the workspace's manifests alone.
# rust-toolchain.toml comes first, so rustup installs the pinned release here and
# everything below builds with it: dependencies cooked with another toolchain
# would all be compiled again by the real build.
FROM docker.io/library/rust:1.99-slim-trixie AS chef
WORKDIR /src
COPY rust-toolchain.toml ./
RUN cargo install cargo-chef --locked --version 0.1.78

# The recipe: the manifests, the lockfile, rust-toolchain.toml and
# .cargo/config.toml, so it only changes when one of them or the crate layout
# does.
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS build
# Dependencies get their own layer, which every commit that leaves the recipe
# alone reuses. The workspace's own crates are empty stand-ins here; the real
# ones are compiled below.
COPY --from=planner /src/recipe.json recipe.json
RUN cargo chef cook --release --locked -p cohesiond -p cohesion --recipe-path recipe.json
COPY . .
COPY --from=web /src/web/build web/build
# What the binaries say they are. Declared here so that a new version doesn't
# cost the dependencies' layer above.
ARG COHESION_VERSION
# target goes too, so this layer holds only the binaries: nothing in it could be
# reused by the next commit, and it would all be uploaded to the build cache.
RUN cargo build --release --locked -p cohesiond -p cohesion \
    && mkdir /out \
    && mv target/release/cohesiond target/release/cohesion /out/ \
    && rm -rf target

FROM docker.io/library/debian:trixie-slim
LABEL org.opencontainers.image.licenses=AGPL-3.0-only
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates tzdata \
    && rm -rf /var/lib/apt/lists/*
RUN useradd --uid 1000 --home-dir /data --no-create-home --shell /usr/sbin/nologin cohesion \
    && install -d -o cohesion -g cohesion /data /run/cohesion
COPY --from=build /out/cohesiond /usr/local/bin/cohesiond
COPY --from=build /out/cohesion /usr/local/libexec/cohesion
COPY scripts/docker-cohesion.sh /usr/local/bin/cohesion
# The licence, and the notices for what the binaries and the embedded web UI
# include (docs/licensing.md).
COPY LICENSE THIRD_PARTY_NOTICES.md /usr/share/doc/cohesion/
COPY --from=web /src/web/build/third-party-licenses.md /usr/share/doc/cohesion/web-third-party-licenses.md
# Where both binaries look by default, for `docker exec` as much as for the
# daemon: the daemon's data and the CLI's config on the volume, under
# cohesiond/ and cohesion/, and the socket off it.
ENV XDG_CONFIG_HOME=/data XDG_RUNTIME_DIR=/run/cohesion
USER cohesion
VOLUME /data
# The daemon's mutual-TLS port, the only thing the image publishes.
EXPOSE 3000
# Arguments to `docker run` after the image name go to cohesiond.
ENTRYPOINT ["cohesiond"]
