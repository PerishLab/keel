# Toolchain image for keel tests against compose postgres.
# Reuses the stock Rust image; does not bake application secrets.
FROM rust:1.96-bookworm

RUN apt-get update \
  && apt-get install -y --no-install-recommends libpq-dev pkg-config \
  && rm -rf /var/lib/apt/lists/*

WORKDIR /src
COPY . .

ENV CARGO_TERM_COLOR=always
RUN cargo test --locked --workspace --no-run

CMD ["cargo", "test", "--locked", "--workspace"]
