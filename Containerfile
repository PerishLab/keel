FROM docker.io/library/debian:trixie-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates && rm -rf /var/lib/apt/lists/*
RUN groupadd --gid 10001 keel \
    && useradd --uid 10001 --gid 10001 --home /keel --shell /usr/sbin/nologin keel
WORKDIR /keel
COPY keel-api /usr/local/bin/keel-api
RUN chmod 0755 /usr/local/bin/keel-api && mkdir -p .local && chown -R keel:keel /keel
USER keel
EXPOSE 8080
ENTRYPOINT ["keel-api"]
