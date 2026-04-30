//! Compile-time embedding of the `jr` Atlassian OAuth app credentials.
//!
//! Reads `JR_BUILD_OAUTH_CLIENT_ID` and `JR_BUILD_OAUTH_CLIENT_SECRET` from
//! the build environment. When both are set, generates a fresh random 32-byte
//! XOR key (per build) and writes `$OUT_DIR/embedded_oauth.rs` with three
//! constants: `EMBEDDED_ID`, `EMBEDDED_SECRET_XOR`, `EMBEDDED_SECRET_KEY`.
//! When either is missing (forks, local `cargo build`), all three are emitted
//! as `None`.
//!
//! XOR obfuscation defeats automated secret scanners (GitHub bots, generic
//! `strings | grep` patterns). It does NOT defeat reverse engineering. The
//! mitigation for a motivated attacker is Atlassian client_secret rotation.

use std::env;
use std::fs;
#[cfg(unix)]
use std::io::Read;
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-env-changed=JR_BUILD_OAUTH_CLIENT_ID");
    println!("cargo:rerun-if-env-changed=JR_BUILD_OAUTH_CLIENT_SECRET");

    let id = env::var("JR_BUILD_OAUTH_CLIENT_ID")
        .ok()
        .filter(|s| !s.is_empty());
    let secret = env::var("JR_BUILD_OAUTH_CLIENT_SECRET")
        .ok()
        .filter(|s| !s.is_empty());

    let out_dir = env::var("OUT_DIR").expect("cargo sets OUT_DIR for build scripts");
    let out_path = Path::new(&out_dir).join("embedded_oauth.rs");

    let body = match (id, secret) {
        (Some(id), Some(secret)) => {
            let key = generate_xor_key();
            let xored: Vec<u8> = secret
                .as_bytes()
                .iter()
                .enumerate()
                .map(|(i, b)| b ^ key[i % 32])
                .collect();
            format!(
                "pub const EMBEDDED_ID: Option<&str> = Some({id:?});\n\
                 pub const EMBEDDED_SECRET_XOR: Option<&[u8]> = Some(&{xored:?});\n\
                 pub const EMBEDDED_SECRET_KEY: Option<&[u8; 32]> = Some(&{key:?});\n"
            )
        }
        _ => "pub const EMBEDDED_ID: Option<&str> = None;\n\
             pub const EMBEDDED_SECRET_XOR: Option<&[u8]> = None;\n\
             pub const EMBEDDED_SECRET_KEY: Option<&[u8; 32]> = None;\n"
            .to_string(),
    };

    fs::write(&out_path, body).expect("write embedded_oauth.rs");
}

/// 32 random bytes from the OS entropy source. Build scripts run on the
/// host's OS, so /dev/urandom (Unix) or BCryptGenRandom (Windows) is
/// available. Other host targets are unsupported by `jr`'s release matrix
/// (macOS / Linux / Windows only) — fail loudly there rather than silently
/// emitting an empty `[u8; 32]`.
#[cfg(not(any(unix, windows)))]
compile_error!(
    "build.rs: unsupported host platform — jr's build pipeline assumes a unix or windows host \
     (the OAuth XOR key needs OS entropy via /dev/urandom or BCryptGenRandom). \
     Add a host-OS branch in generate_xor_key if porting to a new platform."
);

fn generate_xor_key() -> [u8; 32] {
    #[cfg(unix)]
    {
        let mut f = fs::File::open("/dev/urandom").expect("open /dev/urandom");
        let mut buf = [0u8; 32];
        f.read_exact(&mut buf).expect("read /dev/urandom");
        buf
    }
    #[cfg(windows)]
    {
        // BCryptGenRandom via a tiny inline shim — no extra build-deps.
        // Fall back to system_clock-seeded LCG only if BCrypt fails (last-
        // resort; the build host always has BCrypt available on supported
        // Windows versions).
        use std::time::SystemTime;
        let mut buf = [0u8; 32];
        // Try BCrypt first.
        #[link(name = "bcrypt")]
        unsafe extern "system" {
            fn BCryptGenRandom(
                hAlgorithm: *mut std::ffi::c_void,
                pbBuffer: *mut u8,
                cbBuffer: u32,
                dwFlags: u32,
            ) -> i32;
        }
        // SAFETY: BCryptGenRandom writes exactly cbBuffer=32 bytes into
        // pbBuffer. We pass `buf.as_mut_ptr()` for a stack-owned [u8; 32]
        // that outlives the call, with cbBuffer=32 matching the buffer size,
        // and dwFlags=BCRYPT_USE_SYSTEM_PREFERRED_RNG (0x00000002) which
        // tells the API to use the system RNG without requiring a handle.
        let status =
            unsafe { BCryptGenRandom(std::ptr::null_mut(), buf.as_mut_ptr(), 32, 0x00000002) };
        if status == 0 {
            return buf;
        }
        // Fallback (should be unreachable on supported Windows).
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;
        let mut s = nanos;
        for b in buf.iter_mut() {
            s = s
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            *b = (s >> 33) as u8;
        }
        buf
    }
}
