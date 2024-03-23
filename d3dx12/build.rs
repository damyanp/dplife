use std::{
    env,
    io::{stdout, Write},
    path::Path,
    process::Command,
};

fn main() {
    install_nuget();
}

fn install_nuget() {
    let manifest_dir = env::var_os("CARGO_MANIFEST_DIR").unwrap();
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let packages_config = Path::new(&manifest_dir).join("packages.config");
    let output_dir = Path::new(&out_dir).join("packages");

    let nuget_result = Command::new("nuget")
        .args([
            "install",
            packages_config.to_str().unwrap(),
            "-ExcludeVersion",
            "-OutputDirectory",
            output_dir.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to run nuget install");

    if !nuget_result.status.success() {
        stdout().write_all(&nuget_result.stderr).unwrap();
        panic!("nuget install failed");
    }

    println!("cargo::rerun-if-changed=packages.config");
    println!("cargo::rerun-if-changed=build.rs");
}
