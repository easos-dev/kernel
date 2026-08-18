FROM debian:trixie-slim AS builder
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates cargo rustc \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /build
COPY source ./
RUN cargo build --release --locked

FROM debian:trixie-slim AS runtime
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates tini \
    && rm -rf /var/lib/apt/lists/*

ENV EASOS_HOME=/easos
ENV EASOS_RUNTIME_HOME=/run/easos
COPY --from=builder /build/target/release/easos /opt/easos/bin/easos
COPY --from=builder /build/target/release/easos-kerneld /opt/easos/bin/easos-kerneld
COPY source/packaging/install.sh /opt/easos/install.sh
COPY source/packaging/container-entrypoint.sh /opt/easos/container-entrypoint.sh
COPY config/easos-kernel.service /opt/easos/easos-kernel.service
COPY manifest/main.json /opt/easos/plugin/manifest/main.json
COPY config/main.json /opt/easos/plugin/config/main.json
COPY config/state.json /opt/easos/plugin/config/state.json

VOLUME ["/easos"]
ENTRYPOINT ["/usr/bin/tini", "--", "/opt/easos/container-entrypoint.sh"]
