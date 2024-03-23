use std::io::{stdout, Write};

use d3dx12::build::dxc_command;

fn main() {
    let result = dxc_command("renderer/points_renderer.hlsl")
        .output()
        .expect("Failed to run dxc");

    if !result.status.success() {
        stdout().write_all(&result.stderr).unwrap();
        panic!("dxc compile failed");
    }
}
