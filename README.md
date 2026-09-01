# sororail-backend

[![CI](https://github.com/Sororail/sororail-backend/actions/workflows/ci.yml/badge.svg)](https://github.com/Sororail/sororail-backend/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
![Rust 1.89+](https://img.shields.io/badge/rust-1.89%2B-orange.svg)

The API server for the Sororail app. It serves data to
[sororail-frontend](https://github.com/Sororail/sororail-frontend), handles user
registration and authentication, and stores application state in PostgreSQL.

> **Note:** This repository used to hold the SO4 oracle/keeper service. That
> code and its full history now live on the
> [`legacy-oracle`](https://github.com/Sororail/sororail-backend/tree/legacy-oracle)
> branch. `main` holds only the Sororail API server described below.

---

## Table of contents

- [Features](#features)
- [Tech stack](#tech-stack)
- [Architecture](#architecture)
- [Project structure](#project-structure)
- [Getting started](#getting-started)
  - [Prerequisites](#prerequisites)
  - [1. Clone the repository](#1-clone-the-repository)
  - [2. Set up PostgreSQL](#2-set-up-postgresql)
  - [3. Configure the environment](#3-configure-the-environment)
  - [4. Run the server](#4-run-the-server)
  - [5. Smoke-test it](#5-smoke-test-it)
- [Configuration reference](#configuration-reference)
- [Database](#database)
  - [Schema](#schema)
  - [Migrations](#migrations)
- [API reference](#api-reference)
  - [Conventions](#conventions)
  - [GET /health](#get-health)
  - [GET /ready](#get-ready)
  - [POST /auth/register](#post-authregister)
  - [POST /auth/login](#post-authlogin)
  - [GET /auth/me](#get-authme)
  - [Error reference](#error-reference)
- [Authentication model](#authentication-model)
- [Running with Docker](#running-with-docker)
- [Testing](#testing)
- [Continuous integration](#continuous-integration)
- [Deployment notes](#deployment-notes)
- [Security considerations](#security-considerations)
- [Troubleshooting](#troubleshooting)
- [Contributing](#contributing)
- [License](#license)

---

## Features

- **User accounts.** Register and log in with an email and password.
- **Secure password storage.** Passwords are hashed with
  [Argon2](https://en.wikipedia.org/wiki/Argon2) (`argon2id`, default parameters)
  and a unique random salt per user. Plaintext passwords are never stored or logged.
- **Stateless sessions.** Successful auth returns a signed JWT (HS256) that is
  valid for 24 hours.
- **Input validation.** Emails are trimmed, lowercased and checked for a valid
  shape. Passwords must be at least 8 characters.
- **Consistent errors.** All application errors come back as
  `{"error": "..."}` with a meaningful HTTP status code.
- **Health probes.** Separate liveness (`/health`) and readiness (`/ready`,
  which checks the database) endpoints for orchestrators and load balancers.
- **Automatic migrations.** The schema migrates itself on startup.
- **Fail-fast config.** The server refuses to start without required settings
  or with a weak `JWT_SECRET`.
- **Container-ready.** A multi-stage Dockerfile with a built-in `HEALTHCHECK`.

## Tech stack

| Concern          | Choice                                                                 |
|------------------|------------------------------------------------------------------------|
| Language         | Rust (edition 2021, MSRV **1.89**)                                     |
| Async runtime    | [Tokio](https://tokio.rs)                                              |
| HTTP framework   | [Axum 0.7](https://github.com/tokio-rs/axum) + [tower-http](https://github.com/tower-rs/tower-http) (CORS, tracing) |
| Database         | PostgreSQL via [sqlx 0.7](https://github.com/launchbadge/sqlx) (async, rustls) |
| Password hashing | [`argon2`](https://crates.io/crates/argon2)                            |
| Tokens           | [`jsonwebtoken`](https://crates.io/crates/jsonwebtoken) (HS256)        |
| Logging          | [`tracing`](https://crates.io/crates/tracing) + `tracing-subscriber` (`RUST_LOG` filter) |
| Errors           | `thiserror` (typed API errors) + `anyhow` (internal plumbing)          |
| Config           | Environment variables, with optional `.env` loading via `dotenvy`      |

## Architecture

```
                ┌───────────────────────┐
                │  sororail-frontend    │
                └──────────┬────────────┘
                           │ HTTPS / JSON
                           ▼
┌──────────────────────────────────────────────────────────┐
│ sororail-backend (single binary)                         │
│                                                          │
│  main.rs                                                 │
│    load .env → Config::from_env() (validated)            │
│    → PgPool (max 10 connections) → run migrations        │
│    → build Router → listen on BIND_ADDR                  │
│                                                          │
│  Middleware: TraceLayer (request logs) → CorsLayer       │
│                                                          │
│  routes/                                                 │
│    health.rs       GET  /health, /ready                  │
│    auth_routes.rs  POST /auth/register, /auth/login      │
│                    GET  /auth/me                         │
│                                                          │
│  auth.rs   Argon2 hash/verify, JWT issue/verify          │
│  error.rs  AppError → {"error": "..."} + status code     │
└──────────────────────────┬───────────────────────────────┘
                           │ sqlx (TCP)
                           ▼
                 ┌───────────────────┐
                 │    PostgreSQL     │
                 │   users table     │
                 └───────────────────┘
```

Each request goes through the tracing and CORS layers to its handler. Handlers
share an `AppState` holding the connection pool and the loaded `Config`. They
return `Result<T, AppError>`, so every failure is turned into a JSON response
in one place ([`src/error.rs`](src/error.rs)).

## Project structure

```
sororail-backend/
├── .github/
│   └── workflows/ci.yml        # fmt, clippy, tests, MSRV check, Docker build
├── migrations/
│   └── 0001_create_users.sql   # users table
├── src/
│   ├── main.rs                 # entrypoint: config, DB pool, migrations, server
│   ├── config.rs               # env-based Config + validation
│   ├── auth.rs                 # password hashing and JWT helpers
│   ├── error.rs                # AppError and its HTTP mapping
│   ├── models.rs               # User row + request/response types
│   └── routes/
│       ├── mod.rs              # router: path → handler
│       ├── health.rs           # /health, /ready
│       └── auth_routes.rs      # /auth/register, /auth/login, /auth/me
├── .env.example                # template for local configuration
├── .dockerignore
├── Cargo.toml / Cargo.lock
├── Dockerfile                  # multi-stage build → debian:bookworm-slim
├── CONTRIBUTING.md
├── LICENSE                     # MIT
└── README.md
```

## Getting started

### Prerequisites

| Tool       | Version | Notes                                                             |
|------------|---------|-------------------------------------------------------------------|
| Rust       | ≥ 1.89  | Install via [rustup](https://rustup.rs). Run `rustup update` if older. |
| PostgreSQL | ≥ 13    | Needs the built-in `gen_random_uuid()` (core since PG 13).        |
| OpenSSL CLI | any    | Optional. Handy for generating a `JWT_SECRET`.                    |
| Docker     | any     | Optional. Only needed for the container workflow.                 |

### 1. Clone the repository

```sh
git clone https://github.com/Sororail/sororail-backend.git
cd sororail-backend
```

### 2. Set up PostgreSQL

Use whichever option suits you.

**Option A: an existing local Postgres**

```sh
createuser --pwprompt sororail      # enter "sororail" as the password for local dev
createdb --owner sororail sororail
```

**Option B: a throwaway Postgres in Docker**

```sh
docker run -d --name sororail-db \
  -e POSTGRES_USER=sororail \
  -e POSTGRES_PASSWORD=sororail \
  -e POSTGRES_DB=sororail \
  -p 5432:5432 \
  postgres:16
```

Either way, the database matches the default `DATABASE_URL` in `.env.example`.

### 3. Configure the environment

```sh
cp .env.example .env
```

Then generate a signing secret and put it into `.env`:

```sh
echo "JWT_SECRET=$(openssl rand -hex 32)" >> .env
```

Remove the empty `JWT_SECRET=` line the template ships with, or fill it in
directly. Your `.env` should end up looking like this:

```dotenv
BIND_ADDR=0.0.0.0:8080
DATABASE_URL=postgres://sororail:sororail@localhost:5432/sororail
JWT_SECRET=3f9c...64 hex characters...
RUST_LOG=info
```

`.env` is git-ignored. Never commit it.

### 4. Run the server

```sh
cargo run
```

On first start the server applies any pending migrations and then logs:

```
INFO sororail_backend: listening on 0.0.0.0:8080
```

For more detail, including every HTTP request, use
`RUST_LOG=debug,tower_http=debug cargo run`.

### 5. Smoke-test it

```sh
# liveness
curl -i http://localhost:8080/health

# readiness (database reachable?)
curl -i http://localhost:8080/ready

# register
curl -s -X POST http://localhost:8080/auth/register \
  -H 'Content-Type: application/json' \
  -d '{"email":"ada@example.com","password":"correct-horse"}'

# log in and keep the token
TOKEN=$(curl -s -X POST http://localhost:8080/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"email":"ada@example.com","password":"correct-horse"}' \
  | sed -E 's/.*"token":"([^"]+)".*/\1/')

# who am I?
curl -s http://localhost:8080/auth/me -H "Authorization: Bearer $TOKEN"
```

## Configuration reference

All configuration comes from environment variables. If a `.env` file exists in
the working directory, it is loaded first. Variables already set in the real
environment take precedence over `.env`.

| Variable       | Required | Default        | Description                                                                                         |
|----------------|:--------:|----------------|-----------------------------------------------------------------------------------------------------|
| `DATABASE_URL` | **yes**  | none           | Postgres connection string, e.g. `postgres://user:pass@host:5432/db`.                               |
| `JWT_SECRET`   | **yes**  | none           | HMAC key used to sign and verify JWTs. **Must be at least 32 characters**, or the server won't start. |
| `BIND_ADDR`    | no       | `0.0.0.0:8080` | Socket address the HTTP server listens on.                                                          |
| `RUST_LOG`     | no       | (unset)        | [`tracing` filter](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html), e.g. `info`, `debug`, `sororail_backend=debug,tower_http=info`. |

The server exits at startup, before binding the port, if:

| Condition                         | Error message                                   |
|-----------------------------------|-------------------------------------------------|
| `DATABASE_URL` not set            | `DATABASE_URL must be set`                      |
| `JWT_SECRET` not set              | `JWT_SECRET must be set`                        |
| `JWT_SECRET` shorter than 32 chars | `JWT_SECRET must be at least 32 characters`     |
| Database unreachable              | sqlx connection error                           |
| A migration fails                 | sqlx migration error                            |
| `BIND_ADDR` already in use        | `Address already in use`                        |

> **Rotating `JWT_SECRET`** immediately invalidates every token issued so far.
> All users have to log in again.

## Database

### Schema

```sql
CREATE TABLE users (
    id            UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    email         TEXT        NOT NULL UNIQUE,
    password_hash TEXT        NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

| Column          | Notes                                                                 |
|-----------------|-----------------------------------------------------------------------|
| `id`            | Random UUIDv4, generated by Postgres. Used as the JWT `sub` claim.    |
| `email`         | Always stored trimmed and lowercased. The `UNIQUE` constraint enforces one account per email. |
| `password_hash` | Argon2 PHC string (`$argon2id$v=19$...`). Includes its own salt and parameters. Never returned by the API. |
| `created_at`    | Set by the database at insert time (UTC).                             |

### Migrations

Migrations live in [`migrations/`](migrations) and are embedded in the binary at
compile time with `sqlx::migrate!()`. On every startup the server applies any
that haven't run yet and records them in the `_sqlx_migrations` table.

To add a migration, create a new file with the next number:

```
migrations/0002_<short_description>.sql
```

Rules:
- **Never edit a migration that has already been merged.** sqlx checksums
  applied migrations and will refuse to start if one changes.
- Keep each migration focused and forward-only.
- Because migrations are embedded at compile time, rebuild after adding one.

You can also manage migrations by hand with
[`sqlx-cli`](https://crates.io/crates/sqlx-cli) (`cargo install sqlx-cli --no-default-features --features rustls,postgres`),
for example `sqlx migrate info` to see what's been applied. You don't need it
for normal development.

## API reference

### Conventions

- **Base URL (local):** `http://localhost:8080`
- **Request bodies** are JSON and need `Content-Type: application/json`.
- **Response bodies** are JSON, except `/health` (plain text) and `/ready`
  (empty body).
- **Authentication:** protected endpoints expect
  `Authorization: Bearer <token>`, using the token from register or login.
- **Errors** produced by the application look like:
  ```json
  { "error": "human-readable message" }
  ```
  Some malformed requests are rejected by the framework before they reach the
  application, and those responses are plain text (see
  [Error reference](#error-reference)).
- **CORS:** every origin, method and header is currently allowed (see
  [Security considerations](#security-considerations)).

### `GET /health`

Liveness probe. It returns `200` whenever the process is up and serving HTTP,
and does **not** touch the database.

```http
HTTP/1.1 200 OK
content-type: text/plain; charset=utf-8

ok
```

### `GET /ready`

Readiness probe. It runs `SELECT 1` against the database.

| Status | Meaning                          |
|--------|----------------------------------|
| `200`  | Database reachable               |
| `503`  | Database query failed            |

Both responses have an empty body.

### `POST /auth/register`

Creates a user and returns a token, so the client is logged in right away.

**Request**

```json
{
  "email": "ada@example.com",
  "password": "correct-horse"
}
```

| Field      | Type   | Rules                                                                 |
|------------|--------|-----------------------------------------------------------------------|
| `email`    | string | Trimmed and lowercased. Needs a non-empty part before `@` and a `.` in the domain. |
| `password` | string | At least 8 characters.                                                |

**Response: `200 OK`**

```json
{ "token": "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9..." }
```

**Errors**

| Status | Body                                                       | When                               |
|--------|------------------------------------------------------------|------------------------------------|
| `400`  | `{"error":"invalid email address"}`                        | Email fails validation             |
| `400`  | `{"error":"password must be at least 8 characters"}`       | Password too short                 |
| `409`  | `{"error":"email already registered"}`                     | Email already in use (any letter case) |
| `500`  | `{"error":"internal server error"}`                        | Database or hashing failure (details are logged server-side) |

### `POST /auth/login`

Exchanges credentials for a token.

**Request**

```json
{
  "email": "ada@example.com",
  "password": "correct-horse"
}
```

The email is trimmed and lowercased before lookup, so `Ada@Example.com` works.

**Response: `200 OK`**

```json
{ "token": "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9..." }
```

**Errors**

| Status | Body                                  | When                                                    |
|--------|---------------------------------------|---------------------------------------------------------|
| `401`  | `{"error":"invalid credentials"}`     | Unknown email **or** wrong password (deliberately the same response) |
| `500`  | `{"error":"internal server error"}`   | Database failure                                        |

### `GET /auth/me`

Returns the user the bearer token belongs to.

**Request**

```http
GET /auth/me
Authorization: Bearer eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9...
```

**Response: `200 OK`**

```json
{
  "id": "c036c31b-f472-4cd9-8563-08ef6aba048e",
  "email": "ada@example.com",
  "created_at": "2026-11-06T17:48:46.472002Z"
}
```

`password_hash` is never included in the response.

**Errors**

| Status | Body                                  | When                                                             |
|--------|---------------------------------------|------------------------------------------------------------------|
| `401`  | `{"error":"invalid credentials"}`     | Header missing or not `Bearer`; token bad, expired or signed with another secret; or the user no longer exists |
| `500`  | `{"error":"internal server error"}`   | Database failure                                                 |

### Error reference

| Status | Source      | Body format | Typical cause                                                         |
|--------|-------------|-------------|-----------------------------------------------------------------------|
| `400`  | application | JSON        | Validation failed (email or password)                                 |
| `400`  | framework   | plain text  | Body is not valid JSON, e.g. `Failed to parse the request body as JSON: ...` |
| `401`  | application | JSON        | Bad credentials or a missing or invalid token                         |
| `404`  | framework   | empty       | Unknown route                                                         |
| `405`  | framework   | empty       | Wrong HTTP method for an existing route                               |
| `409`  | application | JSON        | Email already registered                                              |
| `415`  | framework   | plain text  | `Content-Type: application/json` header missing                       |
| `422`  | framework   | plain text  | Valid JSON with missing or wrongly typed fields, e.g. ``missing field `password` `` |
| `500`  | application | JSON        | Unexpected server error. The cause is logged at `ERROR` level and never sent to the client |
| `503`  | application | empty       | `/ready` only: database unreachable                                   |

## Authentication model

1. **Registration and login** check the credentials and then call
   `issue_token(user_id, JWT_SECRET)`.
2. **The token** is a JWT signed with **HS256**, with these claims:
   ```json
   { "sub": "<user uuid>", "exp": <unix timestamp, now + 24h> }
   ```
3. **The client** stores the token (memory or secure storage) and sends it as
   `Authorization: Bearer <token>`.
4. **Verification** (`verify_token`) checks the signature against `JWT_SECRET`
   and rejects expired tokens. `jsonwebtoken` allows 60 seconds of clock skew
   by default.
5. **There are no refresh tokens or server-side revocation yet.** A token stays
   valid until it expires or `JWT_SECRET` is rotated. When a token expires,
   the client must log in again.

Password handling:
- Hashing uses `Argon2::default()` (argon2id) with a fresh `OsRng` salt per
  password.
- Verification parses the stored PHC string, so hashes made with different
  parameters in the future keep working.
- Unknown-email and wrong-password logins get the same `401`. This avoids
  saying which emails are registered. Registration does reveal it through `409`,
  which is a deliberate UX trade-off.

## Running with Docker

Build the image:

```sh
docker build -t sororail-backend .
```

The Dockerfile has two stages:

| Stage   | Base image             | Purpose                                                   |
|---------|------------------------|-----------------------------------------------------------|
| builder | `rust:1.89-bookworm`   | `cargo build --release --locked`                          |
| runtime | `debian:bookworm-slim` | Just the binary, plus `ca-certificates` and `curl` (for the healthcheck) |

Run it, pointing at a database the container can reach:

```sh
docker run --rm -p 8080:8080 \
  -e DATABASE_URL=postgres://sororail:sororail@host.docker.internal:5432/sororail \
  -e JWT_SECRET="$(openssl rand -hex 32)" \
  -e RUST_LOG=info \
  sororail-backend
```

Or reuse your `.env`. If Postgres runs on your host machine, change `localhost`
to `host.docker.internal` first:

```sh
docker run --rm -p 8080:8080 --env-file .env sororail-backend
```

The image has a `HEALTHCHECK` that calls `GET /health` every 30 seconds, so
`docker ps` shows the container as `healthy` or `unhealthy`.

## Testing

```sh
cargo test
```

The unit tests need **no database** and cover:

| Module                     | What's tested                                                        |
|----------------------------|----------------------------------------------------------------------|
| `auth`                     | Password hash/verify round trip; JWT round trip; JWT rejected with the wrong secret |
| `config`                   | Short `JWT_SECRET` rejected; secret of sufficient length accepted    |
| `routes::auth_routes`      | Email normalization (trim and lowercase); malformed emails rejected  |

Run the same checks CI runs before you open a PR:

```sh
cargo fmt --all --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
```

To check endpoints end to end, run the server against a local database and use
the [smoke-test commands](#5-smoke-test-it) above.

## Continuous integration

[`.github/workflows/ci.yml`](.github/workflows/ci.yml) runs on every push to
`main` and on every pull request:

| Job              | What it does                                                         |
|------------------|----------------------------------------------------------------------|
| fmt, clippy, test | `cargo fmt --check`, `cargo clippy` with warnings as errors, `cargo test` on stable Rust |
| MSRV (1.89)      | `cargo check` on Rust 1.89, so the declared minimum version really builds |
| Docker build     | Builds the production image so Dockerfile breakage is caught early   |

All jobs use `--locked`, so CI fails if `Cargo.lock` is out of date.

## Deployment notes

- **Probes:** point liveness at `/health` and readiness at `/ready`. An
  instance with no database connection then gets taken out of rotation
  without being restarted.
- **TLS:** the server speaks plain HTTP. Put it behind a TLS-terminating proxy
  or a platform load balancer.
- **Secrets:** inject `JWT_SECRET` and `DATABASE_URL` through your platform's
  secret store, not baked into the image. Every instance must use the **same**
  `JWT_SECRET`, or tokens from one instance will be rejected by another.
- **Scaling:** the service is stateless apart from Postgres, so you can run
  several instances. Each opens up to **10** database connections, so size
  Postgres `max_connections` to match.
- **Migrations** run at startup. When several instances start at once, sqlx
  takes a Postgres advisory lock, so only one applies migrations.
- **Logs** go to stdout in `tracing`'s human-readable format. Control the
  verbosity with `RUST_LOG`.

## Security considerations

Known limitations worth fixing before production. They're good contribution
opportunities:

- **CORS is fully permissive** (`CorsLayer::permissive()`: any origin). Limit
  it to the frontend's origin(s) for production.
- **No rate limiting** on `/auth/login` or `/auth/register`, so credential
  stuffing and brute force are only slowed by Argon2's cost. Add a limiter at
  the proxy or in a tower middleware.
- **No token revocation or refresh flow.** Tokens last their full 24 hours.
- **No email verification or password reset** yet.
- **No request body size limit beyond Axum's default** (2 MB).

To report a vulnerability, please contact the maintainers privately
instead of opening a public issue.

## Troubleshooting

| Symptom                                                    | Likely cause and fix                                                        |
|------------------------------------------------------------|-----------------------------------------------------------------------------|
| `Error: JWT_SECRET must be at least 32 characters`         | Secret too short or still the placeholder. Use `openssl rand -hex 32`.      |
| `Error: DATABASE_URL must be set`                          | No `.env` in the working directory, or the variable is missing from it.     |
| `Connection refused` at startup                            | Postgres isn't running, or the host or port in `DATABASE_URL` is wrong.     |
| `password authentication failed for user ...`              | The credentials in `DATABASE_URL` don't match the Postgres role.            |
| `function gen_random_uuid() does not exist`                | Postgres older than 13. Upgrade, or run `CREATE EXTENSION pgcrypto;`.       |
| `migration ... was previously applied but has been modified` | An already-applied migration file was edited. Revert it and add a new migration instead. |
| `Address already in use`                                   | Something else is on `BIND_ADDR`. Stop it or change the port.               |
| `415 Expected request with Content-Type: application/json` | Add `-H 'Content-Type: application/json'` to the request.                   |
| `/auth/me` always returns 401                              | Missing `Bearer ` prefix, the token expired, or the server restarted with a different `JWT_SECRET`. |
| `rustc ... is not supported by the following packages`     | Your Rust is older than 1.89. Run `rustup update`.                          |
| Docker container can't reach Postgres on the host          | Use `host.docker.internal` instead of `localhost` in `DATABASE_URL`.       |

## Contributing

Contributions are welcome! Please read [CONTRIBUTING.md](CONTRIBUTING.md) for
the workflow: picking up issues, branch and PR conventions, and the checks that
must pass.

In short:

1. Comment on an issue to get assigned before you start.
2. Make focused changes and add tests for any behaviour you change.
3. Run `cargo fmt`, `cargo clippy -- -D warnings` and `cargo test`.
4. Open a PR that references the issue (`Closes #123`) and explains how you
   tested it.

## License

Released under the [MIT License](LICENSE).
