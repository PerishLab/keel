# Data plane (local)

Postgres for adaptor work is declared in the repo root:

- `docker-compose.yml` — official `postgres:16-alpine` only
- `Dockerfile` — Rust tool image that can run tests on the compose network

## Bring up

```sh
docker compose up -d postgres
docker compose ps
```

Default connection (dev only):

```text
postgres://keel:keel@127.0.0.1:5432/keel
```

Export for local cargo when drivers land:

```sh
export KEEL_DATABASE_URL=postgres://keel:keel@127.0.0.1:5432/keel
```

## Tool profile

```sh
docker compose --profile tool run --rm tool cargo test --locked --workspace
```

## Tear down

```sh
docker compose down
# wipe volume:
docker compose down -v
```
