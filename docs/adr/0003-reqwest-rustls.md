# ADR-0003: reqwest with rustls-tls

## Status
Accepted

## Context
reqwest supports multiple TLS backends: `native-tls` (platform default) and `rustls-tls` (pure Rust). The default varies by platform and can be overridden by transitive dependencies pulling in the `default` feature.

## Decision
Use `reqwest` with `default-features = false` and explicitly enable `rustls-tls`.

## Rationale
- **TLS version consistency** — `native-tls` on some platforms negotiates TLSv1.2, which Jira Cloud may reject. `rustls` consistently supports TLSv1.3.
- **Deterministic builds** — disabling default features prevents transitive dependencies from changing the TLS backend
- **No system dependency** — `rustls` is pure Rust, no need for OpenSSL headers on build machines (simplifies cross-compilation)
- **Cross-compilation** — `native-tls` requires platform-specific libraries for each target. `rustls` cross-compiles trivially with `cross-rs`

## Consequences
- Binary size slightly larger than native-tls (~500KB)
- No platform keystore certificate integration (rustls uses its own CA bundle via `webpki-roots`)
- Corporate environments with custom CA certificates need `RUSTLS_NATIVE_CERTS=1` or equivalent configuration
