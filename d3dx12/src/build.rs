use std::{
    env::{self, var},
    fs::copy,
    io::{stderr, stdout, Write},
    path::Path,
    process::Command,
};

pub fn copy_data_file(source_path: &str) {
    println!("!cargo:rerun-if-changed={}", source_path);

    let out_dir = var("OUT_DIR").unwrap();
    let out_dir = Path::new(&out_dir);
    let dest_filename = Path::new(source_path).file_name().expect("dest_filename");
    let dest = out_dir
        .ancestors()
        .nth(3)
        .expect("dest directory")
        .join(dest_filename);

    println!("dest: {}", dest.to_str().expect("to str"));
    copy(source_path, dest).expect("Copy");
}

pub fn dxc_command<P: AsRef<Path>>(source_path: P) -> Command {
    println!(
        "!cargo::rerun-if-changed={}",
        source_path.as_ref().to_str().unwrap()
    );

    static DXC_PATH: &str = concat!(
        env!("OUT_DIR"),
        "/packages/Microsoft.Direct3D.DXC/build/native/bin/x64/dxc.exe"
    );

    let mut command = Command::new(DXC_PATH);
    command.arg(source_path.as_ref());

    command
}

pub fn dxc_compile<P1: AsRef<Path>, P2: AsRef<Path>>(
    source_path: P1,
    dest_path: P2,
    profile: &str,
    entry_point: &str,
) {
    let dest_path = Path::new(&env::var_os("OUT_DIR").unwrap()).join(dest_path);

    let result = dxc_command(source_path)
        .args([
            "-T",
            profile,
            "-E",
            entry_point,
            "-Fo",
            dest_path.to_str().unwrap(),
            "-Od",
            "-Zi",
            "-Qembed_debug"
        ])
        .output()
        .expect("Failed to run dxc");

    stdout().write_all(&result.stdout).unwrap();
    stderr().write_all(&result.stderr).unwrap();

    if !result.status.success() {
        panic!("dxc failed: {:?}", result.status.code());
    }
}
