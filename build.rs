//! Link-time fix for the `embed` feature's C++ dependency.
//!
//! # The problem this exists to solve
//!
//! `--features embed` adds ONNX Runtime through `fastembed` → `ort` → `ort-sys`.
//! ort-sys fetches a prebuilt **static** `libonnxruntime.a`, so the ONNX runtime
//! itself needs no `.so` at runtime. But its build script then emits
//! `cargo:rustc-link-lib=stdc++` (`ort-sys-2.0.0-rc.13`,
//! `build/static_link/mod.rs:21-34`), and that resolves to the **shared**
//! `libstdc++.so.6`. So the embed binary grows a `DT_NEEDED` on the C++ standard
//! library that a Rust binary has no other reason to need, and it dies at load
//! time on a root that has libc, libm and libgcc_s but no libstdc++:
//!
//! ```text
//! error while loading shared libraries: libstdc++.so.6: cannot open shared object file
//! ```
//!
//! The default build does not have this problem and must not acquire it: it is
//! three `DT_NEEDED` entries and a self-contained artifact on a minimal root,
//! which is a documented product property.
//!
//! # The fix, and why it is not a flag
//!
//! A static `libstdc++.a` is linked **instead of** the shared one. Two earlier
//! attempts at this failed, and both failures are informative:
//!
//! - `-static-libstdc++` (via `RUSTFLAGS` or `.cargo/config.toml`) is a **no-op**:
//!   measured 0 bytes of size change and an unchanged `DT_NEEDED` list. It is a
//!   gcc *spec* option, and rustc links through `rust-lld` via the `gcc-ld`
//!   wrapper, which forwards the library list to LLD without expanding specs.
//! - `-C target-feature=+crt-static` does work, but it cannot be applied here:
//!   cargo has no per-feature `rustflags`, so it would land on the default build
//!   too and turn a 3-`NEEDED` dynamic binary into a fully static one. That is a
//!   different product, not a fix to this one.
//!
//! What does work is *position*. rustc already links the bundled ONNX Runtime
//! inside a `-Wl,-Bstatic` group that ends immediately before `-Wl,-Bdynamic`
//! `-lstdc++`, and it already links with `-Wl,--as-needed`. So a static
//! `libstdc++.a` emitted from this build script lands in that group, resolves
//! every C++ symbol ONNX Runtime needs *before* the shared one is reached, and
//! `--as-needed` then declines to record a `DT_NEEDED` for `libstdc++.so.6` at
//! all. Measured on `x86_64-unknown-linux-gnu` / gcc 16.2.1 / rustc 1.98.0:
//! 62,632,504 B and 5 `NEEDED` before, 65,006,360 B and 4 `NEEDED` after, the
//! difference being exactly the C++ runtime code that used to live in the `.so`.
//!
//! `libsupc++.a` is deliberately *not* linked: modern `libstdc++.a` already
//! contains the C++ ABI objects, and adding it changes the binary by 0 bytes.
//!
//! # The fourth `NEEDED`: `ld-linux-x86-64.so.2`
//!
//! The embed binary carries **four** `DT_NEEDED`, not three: `libgcc_s.so.1`,
//! `libm.so.6`, `libc.so.6`, and the dynamic loader itself. The loader entry is
//! the loader appearing in the *dependency list* rather than only in the
//! `PT_INTERP` header, and it is **caused by `__tls_get_addr`**, not by the C++
//! link. Traced on the real link line (`cc` wrapper capturing `"$@"`, then
//! re-running that line directly), on this tree:
//!
//! - The static C++ objects use the general-dynamic TLS model, so they carry an
//!   undefined `__tls_get_addr`: 2 members of the bundled static `libstdc++`
//!   (`eh_globals.o`, `mutex.o`) and 25 members of the static ONNX Runtime
//!   archive (`threadpool.cc.o`, `arena.cc.o`, `inference_session.cc.o`, …).
//! - The output's undefined dynamic symbols contain
//!   `__tls_get_addr@GLIBC_2.3`.
//! - Since glibc 2.34 `__tls_get_addr` is no longer exported by `libc.so.6`. The
//!   only provider among the link's shared objects is
//!   `/usr/lib/ld-linux-x86-64.so.2`, which `-lc` reaches through the Debian
//!   `libc.so` linker script:
//!   `GROUP ( … AS_NEEDED ( /usr/lib/ld-linux-x86-64.so.2 ) )`.
//! - `--as-needed` therefore *keeps* it, because the symbol really is used.
//!   Deleting `-lstdc++` from the link line does not help (still 4 `NEEDED`,
//!   measured); adding a local definition of `__tls_get_addr` does, dropping the
//!   output to exactly the default build's 3 (measured).
//!
//! The default build has no C++ objects, so it has no `__tls_get_addr`, so
//! `AS_NEEDED` drops the entry — which is the whole difference between "three
//! dependencies" and "four".
//!
//! It is benign, and **not worth removing**: the loader is the `PT_INTERP` of
//! every dynamically linked process, so the entry requires nothing that is not
//! already required to start the binary, and the embed build's minimal-root
//! sandbox binds `ld-linux-x86-64.so.2` anyway. Removing it would take one of
//! three changes, none of which is a fix to this build: linking the whole binary
//! statically (`-C target-feature=+crt-static`, which cargo cannot scope to one
//! feature — see above), rebuilding the vendored ONNX archive and `libstdc++`
//! with `-ftls-model=initial-exec` so no general-dynamic TLS remains, or
//! shipping a correct local `__tls_get_addr` — of which there is none outside the
//! loader, which is the entire reason the entry exists. The entry is also
//! evidence rather than noise: a `__tls_get_addr` call is C++ runtime code
//! actually executing.
//!
//! # Scope
//!
//! Gated on `#[cfg(feature = "embed")]` and on the guard below, so the default
//! build is untouched — this script compiles to nothing and emits nothing when
//! the feature is off. The full record, including the measurements and the
//! options that did not work, is `docs/CONSISTENCY.md` §16.

// The default build compiles this file to an empty `main`, so the static-link
// machinery — imports included — is behind the same feature gate.
#[cfg(feature = "embed")]
use std::env;
#[cfg(feature = "embed")]
use std::path::Path;
#[cfg(feature = "embed")]
use std::process::Command;

/// Resolve the C++ standard library and link it statically.
///
/// Nothing is emitted unless the `embed` feature is on, so a default
/// `cargo build --release` produces the same bytes it did before this file
/// existed.
#[cfg(feature = "embed")]
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=CC");

    let target = env_or_empty("TARGET");
    let host = env_or_empty("HOST");
    if let Err(why) = static_libstdcxx_allowed(
        &target,
        &host,
        &env_or_empty("CARGO_CFG_TARGET_OS"),
        &env_or_empty("CARGO_CFG_TARGET_ENV"),
        &env_or_empty("CARGO_CFG_TARGET_ARCH"),
    ) {
        warn(&why);
        return;
    }

    // `cc -print-file-name=libstdc++.a` is the toolchain's own answer to "where
    // is the static C++ runtime", which keeps the gcc version in the path rather
    // than hardcoding one. An unresolved result comes back as the bare filename.
    //
    // Asking the *host* compiler is correct here and only here: the guard above
    // has already established `TARGET == HOST`, so the host compiler is by
    // construction the target compiler, and the archive it names is the target
    // architecture's. For any other target this line is never reached — a
    // cross-compiling sysroot resolver is deliberately not attempted, because a
    // wrong-but-plausible answer there would silently link host objects into a
    // foreign-ABI binary, which is worse than a loud warning.
    let cc = env::var("CC").unwrap_or_else(|_| String::from("cc"));
    let Ok(out) = Command::new(&cc)
        .arg("-print-file-name=libstdc++.a")
        .output()
    else {
        warn(&format!("{cc} could not be run"));
        return;
    };
    let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let path = Path::new(&path);
    if !path.is_absolute() || !path.is_file() {
        warn(&format!(
            "{cc} reported no usable libstdc++.a ({})",
            path.display()
        ));
        return;
    }
    let dir = path.parent().expect("absolute path has a parent");

    println!("cargo:rustc-link-search=native={}", dir.display());
    println!("cargo:rustc-link-lib=static=stdc++");
}

/// The static-`libstdc++` link applies only to a glibc-ABI target that *is* the
/// host. Pure, so it can be exercised without a cargo build; `main` only turns
/// its verdict into cargo directives.
///
/// `Err` carries a plain-English reason naming the condition that failed, because
/// a wrong link here is silent — the link succeeds and the binary simply will
/// not start on a root without `libstdc++.so.6`.
#[cfg(any(feature = "embed", test))]
fn static_libstdcxx_allowed(
    target: &str,
    host: &str,
    target_os: &str,
    target_env: &str,
    target_arch: &str,
) -> Result<(), String> {
    if target_os != "linux" || target_env != "gnu" {
        return Err(format!(
            "target {target} is not a glibc-ABI target (os={target_os}, env={target_env}); \
             there is no host libstdc++ for it"
        ));
    }
    if target != host {
        return Err(format!(
            "target {target} ({target_arch}) is not the host triple ({host}); \
             the host compiler's libstdc++.a is built for {host}, not {target_arch}"
        ));
    }
    Ok(())
}

/// Say why the static link is not happening and leave the build to the
/// toolchain's default (a `libstdc++.so.6` `DT_NEEDED`) rather than breaking a
/// build that used to work.
#[cfg(feature = "embed")]
fn warn(why: &str) {
    println!(
        "cargo:warning=memory-wire: not statically linking libstdc++ — {why}. The embed binary \
         will carry a dynamic libstdc++.so.6 dependency and will not start on a root without it. \
         See docs/CONSISTENCY.md §16."
    );
}

/// Read a cargo-provided variable, treating an absent one as empty rather than
/// unwrapping: the guard below rejects the empty value with a readable reason.
#[cfg(feature = "embed")]
fn env_or_empty(key: &str) -> String {
    env::var(key).unwrap_or_default()
}

/// The default build links nothing through this script.
#[cfg(not(feature = "embed"))]
fn main() {}

#[cfg(test)]
mod tests {
    use super::static_libstdcxx_allowed as allowed;

    #[test]
    fn host_gnu_linux_is_allowed() {
        assert!(allowed(
            "x86_64-unknown-linux-gnu",
            "x86_64-unknown-linux-gnu",
            "linux",
            "gnu",
            "x86_64"
        )
        .is_ok());
    }

    #[test]
    fn non_gnu_linux_is_rejected_as_abi() {
        let e = allowed("aarch64-unknown-linux-musl", "aarch64-unknown-linux-musl", "linux", "musl", "aarch64")
            .expect_err("musl has no host libstdc++ to link");
        assert!(e.contains("glibc-ABI"), "{e}");
        // The arch check must not be what rejected it: musl fails on ABI first.
        assert!(!e.contains("not the host triple"), "{e}");
    }

    #[test]
    fn non_linux_is_rejected_as_abi() {
        let e = allowed("aarch64-apple-darwin", "aarch64-apple-darwin", "macos", "", "aarch64")
            .expect_err("macOS has no libstdc++ at all");
        assert!(e.contains("glibc-ABI"), "{e}");
    }

    #[test]
    fn cross_linux_gnu_is_rejected_as_wrong_archive() {
        let e = allowed(
            "aarch64-unknown-linux-gnu",
            "x86_64-unknown-linux-gnu",
            "linux",
            "gnu",
            "aarch64",
        )
        .expect_err("a host archive would be the wrong architecture");
        assert!(e.contains("not the host triple"), "{e}");
        assert!(e.contains("aarch64"), "{e}");
        // ABI is fine, so the failure must name the triple, not the ABI.
        assert!(!e.contains("glibc-ABI"), "{e}");
    }

    #[test]
    fn missing_cargo_cfg_vars_reject_rather_than_link() {
        // An absent CARGO_CFG_TARGET_ENV must not read as "gnu".
        assert!(allowed("", "", "", "", "").is_err());
    }
}
