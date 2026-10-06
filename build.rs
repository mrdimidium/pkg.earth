// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: MPL-2.0

fn main() {
    dimidiumlabs_ui_build::build("APPLICATION", &["src/styles", "src/ui"], &["src/assets"])
        .expect("failed to compile UI styles");
    println!("cargo::rerun-if-changed=Cargo.lock");
    println!("cargo::rerun-if-changed=mise.toml");
    println!("cargo::rerun-if-changed=mise.lock");

    let out_dir = std::env::var("OUT_DIR").unwrap();
    let target = std::env::var("TARGET").unwrap();
    let output = format!("{out_dir}/licenses.json");

    let status = std::process::Command::new("mise")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args([
            "run",
            "licenses-json",
            "--",
            "--manifest-path",
            "Cargo.toml",
            "--output",
            &output,
            "--target",
            &target,
        ])
        .status()
        .expect("failed to run licenses-json task");

    assert!(status.success(), "failed to generate licenses.json");
}
