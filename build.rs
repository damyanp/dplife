use d3dx12::build::get_dxc_path;

use std::{
    env::{self},
    io::{stderr, stdout, Write},
    path::{Path, PathBuf},
    process::Command,
};

fn main() {
    let mut dxc = DXCCompiler::new(get_dxc_path());

    dxc.compile(
        "src/renderer/points_renderer.hlsl",
        "points_renderer.vs.dxil",
        "vs_6_0",
        "vs_main",
    );
    dxc.compile(
        "src/renderer/points_renderer.hlsl",
        "points_renderer.ps.dxil",
        "ps_6_0",
        "ps_main",
    );
    dxc.compile(
        "src/renderer/points_renderer.hlsl",
        "points_renderer.root_signature",
        "rootsig_1_0",
        "ROOT_SIGNATURE",
    );

    let particle_life = "src/particle_life/particle_life.hlsl";

    dxc.compile(
        particle_life,
        "particle_life.root_signature",
        "rootsig_1_0",
        "ROOT_SIGNATURE",
    );
    dxc.compile(particle_life, "particle_life.dxil", "cs_6_0", "main");
}

pub struct DXCCompiler {
    path: PathBuf
}

impl DXCCompiler {
    pub fn new<P : Into<PathBuf>>(path: P) -> DXCCompiler {
        DXCCompiler { path: path.into() }
    }

    pub fn compile<P1 : AsRef<Path>, P2 : AsRef<Path>>(
        &mut self,
        source_path: P1,
        dest_path: P2,
        profile: &str,
        entry_point: &str,
    ) {
        let dest_path = Path::new(
            &env::var_os("OUT_DIR").unwrap()).join(dest_path);

        let mut command = Command::new(&self.path);

        let result = command
            .arg(source_path.as_ref())
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
}
