use std::{env, io::{stdout, Write}, path::Path, process::Command};

use windows::Win32::Graphics::Direct3D12::{
    ID3D12Device, ID3D12PipelineState, ID3D12RootSignature,
};

pub struct PointsRenderer {
    rs: ID3D12RootSignature,
    pso: ID3D12PipelineState,
}

impl PointsRenderer {
    pub fn new(device: &ID3D12Device) -> Self {
        let rs = Self::create_root_signature(device);
        let pso = Self::create_pipeline_state(device, &rs);
        PointsRenderer { rs, pso }
    }

    fn create_root_signature(device: &ID3D12Device) -> ID3D12RootSignature {
        unreachable!()
    }

    fn create_pipeline_state(
        device: &ID3D12Device,
        rs: &ID3D12RootSignature,
    ) -> ID3D12PipelineState {
        unreachable!()
    }
}

