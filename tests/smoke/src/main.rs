// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: MPL-2.0

// Black-box smoke tests for pkg.earth.
//
// Downloads archives from pkg.earth and upstream, compares sha256 hashes,
// verifies that minisig signatures (Zig) and sha256 checksums (Go) match upstream.
//
// Usage:
//   PKG_EARTH_URL=https://pkg.earth cargo run -p smoke

use sha2::{Digest, Sha256};

const GO_FILES: &[&str] = &[
    "go1.23.0.linux-amd64.tar.gz",
    "go1.23.0.darwin-arm64.tar.gz",
    "go1.21.0.windows-amd64.zip",
];

struct ZigTestFile {
    file: &'static str,
    version: &'static str,
}

const ZIG_FILES: &[ZigTestFile] = &[
    // 0.15.2 (new tarball name format, some new targets)
    ZigTestFile {
        file: "zig-0.15.2.tar.xz",
        version: "0.15.2",
    },
    ZigTestFile {
        file: "zig-x86_64-windows-0.15.2.zip",
        version: "0.15.2",
    },
    ZigTestFile {
        file: "zig-aarch64-macos-0.15.2.tar.xz",
        version: "0.15.2",
    },
    ZigTestFile {
        file: "zig-aarch64-netbsd-0.15.2.tar.xz",
        version: "0.15.2",
    },
    ZigTestFile {
        file: "zig-powerpc64le-freebsd-0.15.2.tar.xz",
        version: "0.15.2",
    },
    // Every 'zig-xxx-linux-0.14.1.tar.xz' (same order as on the website)
    ZigTestFile {
        file: "zig-x86_64-linux-0.14.1.tar.xz",
        version: "0.14.1",
    },
    ZigTestFile {
        file: "zig-aarch64-linux-0.14.1.tar.xz",
        version: "0.14.1",
    },
    ZigTestFile {
        file: "zig-armv7a-linux-0.14.1.tar.xz",
        version: "0.14.1",
    },
    ZigTestFile {
        file: "zig-riscv64-linux-0.14.1.tar.xz",
        version: "0.14.1",
    },
    ZigTestFile {
        file: "zig-powerpc64le-linux-0.14.1.tar.xz",
        version: "0.14.1",
    },
    ZigTestFile {
        file: "zig-x86-linux-0.14.1.tar.xz",
        version: "0.14.1",
    },
    ZigTestFile {
        file: "zig-loongarch64-linux-0.14.1.tar.xz",
        version: "0.14.1",
    },
    ZigTestFile {
        file: "zig-s390x-linux-0.14.1.tar.xz",
        version: "0.14.1",
    },
    // 0.10.1 (last stage1 release)
    ZigTestFile {
        file: "zig-0.10.1.tar.xz",
        version: "0.10.1",
    },
    ZigTestFile {
        file: "zig-bootstrap-0.10.1.tar.xz",
        version: "0.10.1",
    },
    ZigTestFile {
        file: "zig-linux-i386-0.10.1.tar.xz",
        version: "0.10.1",
    },
    ZigTestFile {
        file: "zig-macos-aarch64-0.10.1.tar.xz",
        version: "0.10.1",
    },
    ZigTestFile {
        file: "zig-windows-x86_64-0.10.1.zip",
        version: "0.10.1",
    },
    // 0.7.1 (oldest supported patch release)
    ZigTestFile {
        file: "zig-0.7.1.tar.xz",
        version: "0.7.1",
    },
    ZigTestFile {
        file: "zig-linux-x86_64-0.7.1.tar.xz",
        version: "0.7.1",
    },
    // 0.6.0 (oldest supported version)
    ZigTestFile {
        file: "zig-0.6.0.tar.xz",
        version: "0.6.0",
    },
    ZigTestFile {
        file: "zig-linux-x86_64-0.6.0.tar.xz",
        version: "0.6.0",
    },
    // 0.1.1 (first release)
    ZigTestFile {
        file: "zig-win64-0.1.1.zip",
        version: "0.1.1",
    },
];

struct Runner {
    passed: u32,
    failed: u32,
    client: reqwest::blocking::Client,
}

impl Runner {
    fn new() -> Self {
        Self {
            passed: 0,
            failed: 0,
            client: reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(600))
                .build()
                .expect("failed to create HTTP client"),
        }
    }

    fn ok(&mut self, name: &str) {
        self.passed += 1;
        println!("  \x1b[32mok\x1b[0m  {name}");
    }

    fn fail(&mut self, name: &str, reason: &str) {
        self.failed += 1;
        println!("  \x1b[31mFAIL\x1b[0m {name}");
        println!("       {reason}");
    }

    fn fetch_bytes(&self, url: &str) -> Result<Vec<u8>, String> {
        let resp = self.client.get(url).send().map_err(|e| format!("{e}"))?;
        if !resp.status().is_success() {
            return Err(format!("{} for {url}", resp.status()));
        }
        resp.bytes().map(|b| b.to_vec()).map_err(|e| format!("{e}"))
    }

    fn fetch_status(&self, url: &str) -> Result<u16, String> {
        let resp = self.client.get(url).send().map_err(|e| format!("{e}"))?;
        Ok(resp.status().as_u16())
    }

    fn fetch_text(&self, url: &str) -> Result<(u16, String, String), String> {
        let resp = self.client.get(url).send().map_err(|e| format!("{e}"))?;
        let status = resp.status().as_u16();
        let content_type = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        let body = resp.text().map_err(|e| format!("{e}"))?;
        Ok((status, content_type, body))
    }
}

fn sha256hex(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}

fn zig_upstream_url(file: &str, version: &str) -> String {
    format!("https://ziglang.org/download/{version}/{file}")
}

fn fingerprinted_asset_path(document: &str, name: &str, extension: &str) -> Option<String> {
    let prefix = format!("/-/assets/{name}.");
    let suffix = format!(".{extension}");
    document.match_indices(&prefix).find_map(|(start, _)| {
        let end = start.checked_add(prefix.len() + 16 + suffix.len())?;
        let path = document.get(start..end)?;
        let fingerprint = path.strip_prefix(&prefix)?.strip_suffix(&suffix)?;
        (fingerprint.len() == 16
            && fingerprint
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()))
        .then(|| path.to_owned())
    })
}

fn test_web(r: &mut Runner, base_url: &str) {
    println!("\n--- Web ---");

    let document = {
        let name = "GET / returns html";
        match r.fetch_text(&format!("{base_url}/")) {
            Ok((status, ct, body)) => {
                if status != 200 {
                    r.fail(name, &format!("status {status}, expected 200"));
                    None
                } else if !ct.contains("text/html") {
                    r.fail(name, &format!("content-type: {ct}"));
                    None
                } else if !body.contains("pkg.earth") {
                    r.fail(name, "body missing \"pkg.earth\"");
                    None
                } else if fingerprinted_asset_path(&body, "foundation", "css").is_none()
                    || fingerprinted_asset_path(&body, "application", "css").is_none()
                    || !body.contains("/-/assets/manifest.webmanifest")
                {
                    r.fail(name, "body missing shared asset URLs");
                    None
                } else {
                    r.ok(name);
                    Some(body)
                }
            }
            Err(e) => {
                r.fail(name, &e);
                None
            }
        }
    };

    for (asset, expected) in [
        (("foundation", "css"), "IBM Plex Sans"),
        (("application", "css"), "--color-text"),
    ] {
        let Some(path) = document
            .as_deref()
            .and_then(|body| fingerprinted_asset_path(body, asset.0, asset.1))
        else {
            continue;
        };
        let name = format!("GET {path} returns css");
        match r.fetch_text(&format!("{base_url}{path}")) {
            Ok((status, ct, body)) => {
                if status != 200 {
                    r.fail(&name, &format!("status {status}"));
                } else if !ct.contains("text/css") {
                    r.fail(&name, &format!("content-type: {ct}"));
                } else if !body.contains(expected) {
                    r.fail(&name, &format!("body missing {expected:?}"));
                } else {
                    r.ok(&name);
                }
            }
            Err(e) => r.fail(&name, &e),
        }
    }

    for path in [
        "/favicon.ico",
        "/apple-touch-icon.png",
        "/robots.txt",
        "/-/assets/favicon.svg",
        "/-/assets/manifest.webmanifest",
        "/-/assets/icon-192.png",
        "/-/assets/icon-512.png",
        "/-/assets/fonts/ibm-plex-math-1.1.0-regular.woff2",
    ] {
        let name = format!("GET {path} returns an asset");
        match r.fetch_status(&format!("{base_url}{path}")) {
            Ok(200) => r.ok(&name),
            Ok(s) => r.fail(&name, &format!("status {s}")),
            Err(e) => r.fail(&name, &e),
        }
    }

    {
        let name = "GET /about/licenses returns html";
        match r.fetch_text(&format!("{base_url}/about/licenses")) {
            Ok((status, ct, body)) => {
                if status != 200 {
                    r.fail(name, &format!("status {status}"));
                } else if !ct.contains("text/html") {
                    r.fail(name, &format!("content-type: {ct}"));
                } else if !body.contains("MPL-2.0") {
                    r.fail(name, "body missing \"MPL-2.0\"");
                } else {
                    r.ok(name);
                }
            }
            Err(e) => r.fail(name, &e),
        }
    }

    {
        let name = "GET /does-not-exist returns 404";
        match r.fetch_status(&format!("{base_url}/does-not-exist.xyz")) {
            Ok(404) => r.ok(name),
            Ok(s) => r.fail(name, &format!("status {s}, expected 404")),
            Err(e) => r.fail(name, &e),
        }
    }
}

fn test_go(r: &mut Runner, base_url: &str) {
    println!("\n--- Go ---");

    for file in GO_FILES {
        // Archive: compare sha256 with upstream
        {
            let name = format!("go: {file} matches upstream");
            match (
                r.fetch_bytes(&format!("{base_url}/go/{file}")),
                r.fetch_bytes(&format!("https://go.dev/dl/{file}")),
            ) {
                (Ok(mirror), Ok(upstream)) => {
                    let zh = sha256hex(&mirror);
                    let uh = sha256hex(&upstream);
                    if zh != uh {
                        r.fail(&name, &format!("mirror={zh} upstream={uh}"));
                    } else {
                        r.ok(&name);
                    }
                }
                (Err(e), _) | (_, Err(e)) => r.fail(&name, &e),
            }
        }

        // Checksum: compare .sha256 endpoint with computed hash
        {
            let name = format!("go: {file}.sha256 matches content");
            match (
                r.fetch_bytes(&format!("{base_url}/go/{file}")),
                r.fetch_text(&format!("{base_url}/go/{file}.sha256")),
            ) {
                (Ok(archive), Ok((200, _, checksum_body))) => {
                    let computed = sha256hex(&archive);
                    let expected = checksum_body.trim();
                    if computed != expected {
                        r.fail(&name, &format!("computed={computed} endpoint={expected}"));
                    } else {
                        r.ok(&name);
                    }
                }
                (Ok(_), Ok((s, _, _))) => {
                    r.fail(&name, &format!("sha256 endpoint returned {s}"));
                }
                (Err(e), _) | (_, Err(e)) => r.fail(&name, &e),
            }
        }
    }

    {
        let name = "go: nonexistent version returns 404";
        match r.fetch_status(&format!("{base_url}/go/go99.99.99.linux-amd64.tar.gz")) {
            Ok(404) => r.ok(name),
            Ok(s) => r.fail(name, &format!("status {s}, expected 404")),
            Err(e) => r.fail(name, &e),
        }
    }
}

fn test_zig(r: &mut Runner, base_url: &str) {
    println!("\n--- Zig ---");

    for entry in ZIG_FILES {
        // Archive: compare sha256 with upstream
        {
            let name = format!("zig: {} matches upstream", entry.file);
            match (
                r.fetch_bytes(&format!("{base_url}/zig/{}", entry.file)),
                r.fetch_bytes(&zig_upstream_url(entry.file, entry.version)),
            ) {
                (Ok(mirror), Ok(upstream)) => {
                    let zh = sha256hex(&mirror);
                    let uh = sha256hex(&upstream);
                    if zh != uh {
                        r.fail(&name, &format!("mirror={zh} upstream={uh}"));
                    } else {
                        r.ok(&name);
                    }
                }
                (Err(e), _) | (_, Err(e)) => r.fail(&name, &e),
            }
        }

        // Signature: compare .minisig with upstream
        {
            let name = format!("zig: {}.minisig matches upstream", entry.file);
            let mirror_url = format!("{base_url}/zig/{}.minisig", entry.file);
            let upstream_url = format!("{}.minisig", zig_upstream_url(entry.file, entry.version));
            match (r.fetch_bytes(&mirror_url), r.fetch_bytes(&upstream_url)) {
                (Ok(mirror), Ok(upstream)) => {
                    if mirror != upstream {
                        r.fail(&name, "minisig content differs from upstream");
                    } else {
                        r.ok(&name);
                    }
                }
                (Err(e), _) | (_, Err(e)) => r.fail(&name, &e),
            }
        }
    }

    {
        let name = "zig: nonexistent version returns 404";
        match r.fetch_status(&format!("{base_url}/zig/zig-x86_64-linux-99.99.99.tar.xz")) {
            Ok(404) => r.ok(name),
            Ok(s) => r.fail(name, &format!("status {s}, expected 404")),
            Err(e) => r.fail(name, &e),
        }
    }
}

fn main() {
    let base_url = match std::env::var("PKG_EARTH_URL") {
        Ok(url) => url,
        Err(_) => {
            eprintln!("Error: PKG_EARTH_URL environment variable is not set.");
            eprintln!("Usage: PKG_EARTH_URL=https://pkg.earth cargo run -p smoke");
            std::process::exit(1);
        }
    };

    let base_url = base_url.trim_end_matches('/');
    println!("Smoke tests against {base_url}");

    let mut runner = Runner::new();

    test_web(&mut runner, base_url);
    test_go(&mut runner, base_url);
    test_zig(&mut runner, base_url);

    println!(
        "\n--- Results: {} passed, {} failed ---",
        runner.passed, runner.failed
    );

    if runner.failed > 0 {
        std::process::exit(1);
    }
}
