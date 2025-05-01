use std::{
    env::var,
    fs::copy,
    path::{Path, PathBuf},
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

pub fn get_dxc_path() -> PathBuf {
    static DXC_PATH: &str = concat!(
        env!("OUT_DIR"),
        "/packages/Microsoft.Direct3D.DXC/build/native/bin/x64/dxc.exe"
    );
    Path::new(DXC_PATH).to_path_buf()
}
