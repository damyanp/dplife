use d3dx12::build::dxc;

fn main() {
    dxc("renderer/points_renderer.hlsl").output().unwrap();
}