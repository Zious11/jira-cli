# ADR-0003: reqwest with rustls-tls

## Status
Accepted

## Context
reqwest supports multiple TLS backends: `native-tls` (platform default) and `rustls-tls` (pure Rust). The default varies by platform and can be overridden by transitive dependencies pulling in the `default` feature.

## Decision
Use `reqwest` with `default-features = false` and explicitly enable rustls. Feature name changed from `rustls-tls` (0.12) to `rustls` (0.13).

## Rationale
- **TLS version consistency** — `native-tls` on some platforms negotiates TLSv1.2, which Jira Cloud may reject. `rustls` consistently supports TLSv1.3.
- **Deterministic builds** — disabling default features prevents transitive dependencies from changing the TLS backend
- **No system dependency** — `rustls` is pure Rust, no need for OpenSSL headers on build machines (simplifies cross-compilation)
- **Cross-compilation** — `native-tls` requires platform-specific libraries for each target. `rustls` cross-compiles trivially with `cross-rs`

## Consequences
- Binary size slightly larger than native-tls (~500KB)
- **Certificate-root verification uses the platform trust store** (updated for reqwest 0.13): the `rustls` default in reqwest 0.13 delegates root-CA verification to `rustls-platform-verifier`, which reads the OS trust store (macOS Security framework, Windows CryptoAPI/certificate store, Linux system roots). The TLS handshake and crypto remain pure Rust — no OpenSSL, no Schannel for the protocol itself. This replaces the prior reqwest 0.12 behavior of bundling Mozilla roots via `webpki-roots`. As a result, enterprise CAs trusted in the OS store work automatically; `RUSTLS_NATIVE_CERTS=1` is no longer needed for that case. The `rustls-tls-webpki-roots` feature remains available as an opt-in to restore bundled-Mozilla-roots behavior (zero OS-store reliance) at the cost of enterprise-CA and CRL support.
- Corporate environments with custom CA certificates should install the CA into the OS trust store (the platform verifier picks it up automatically under reqwest 0.13)
