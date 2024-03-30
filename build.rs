use d3dx12::build::dxc_compile;

fn main() {
    dxc_compile(
        "src/renderer/points_renderer.hlsl",
        "points_renderer.vs.dxil",
        "vs_6_0",
        "vs_main",
    );
    dxc_compile(
        "src/renderer/points_renderer.hlsl",
        "points_renderer.ps.dxil",
        "ps_6_0",
        "ps_main",
    );
    dxc_compile(
        "src/renderer/points_renderer.hlsl",
        "points_renderer.root_signature",
        "rootsig_1_0",
        "ROOT_SIGNATURE",
    );

    let particle_life = "src/particle_life/particle_life.hlsl";

    dxc_compile(
        particle_life,
        "particle_life.root_signature",
        "rootsig_1_0",
        "ROOT_SIGNATURE",
    );
    dxc_compile(particle_life, "particle_life.dxil", "cs_6_0", "main");
}
