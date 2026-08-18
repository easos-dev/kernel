FROM debian:trixie-slim AS builder
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates cargo rustc \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /build
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY src ./src
RUN cargo build --release --locked

FROM debian:trixie-slim AS runtime
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates tini \
    && rm -rf /var/lib/apt/lists/*

ENV EASOS_HOME=/var/lib/easos
COPY --from=builder /build/target/release/easos /opt/easos/bin/easos
COPY --from=builder /build/target/release/easos-kerneld /opt/easos/bin/easos-kerneld
COPY packaging/install.sh /opt/easos/install.sh
COPY packaging/container-entrypoint.sh /opt/easos/container-entrypoint.sh
COPY packaging/easos-kernel.service /opt/easos/easos-kernel.service

VOLUME ["/var/lib/easos"]
ENTRYPOINT ["/usr/bin/tini", "--", "/opt/easos/container-entrypoint.sh"]
