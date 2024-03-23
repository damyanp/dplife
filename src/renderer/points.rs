use windows::{
    core::s,
    Win32::Graphics::{
        Direct3D12::{
            ID3D12Device, ID3D12PipelineState, ID3D12RootSignature,
            D3D12_GRAPHICS_PIPELINE_STATE_DESC, D3D12_INPUT_ELEMENT_DESC, D3D12_INPUT_LAYOUT_DESC,
            D3D12_PRIMITIVE_TOPOLOGY_TYPE_POINT,
        },
        Dxgi::Common::{
            DXGI_FORMAT, DXGI_FORMAT_R32G32_FLOAT, DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_SAMPLE_DESC,
        },
    },
};

use d3dx12::{BlendDesc, RasterizerDesc, ShaderBytecode};

pub struct PointsRenderer {
    rs: ID3D12RootSignature,
    pso: ID3D12PipelineState,
}

impl PointsRenderer {
    pub fn new(device: &ID3D12Device, rtv_format: DXGI_FORMAT) -> Self {
        let rs = Self::create_root_signature(device);
        let pso = Self::create_pipeline_state(device, rtv_format, &rs);
        PointsRenderer { rs, pso }
    }

    fn create_root_signature(device: &ID3D12Device) -> ID3D12RootSignature {
        let rs = include_bytes!(concat!(env!("OUT_DIR"), "/points_renderer.root_signature"));
        unsafe { device.CreateRootSignature(0, rs).unwrap() }
    }

    fn create_pipeline_state(
        device: &ID3D12Device,
        rtv_format: DXGI_FORMAT,
        rs: &ID3D12RootSignature,
    ) -> ID3D12PipelineState {
        let vs_dxil = include_bytes!(concat!(env!("OUT_DIR"), "/points_renderer.vs.dxil"));
        let ps_dxil = include_bytes!(concat!(env!("OUT_DIR"), "/points_renderer.ps.dxil"));

        let input_layout = [
            D3D12_INPUT_ELEMENT_DESC {
                SemanticName: s!("POSITION"),
                Format: DXGI_FORMAT_R32G32_FLOAT,
                AlignedByteOffset: 0,
                ..Default::default()
            },
            D3D12_INPUT_ELEMENT_DESC {
                SemanticName: s!("COLOR"),
                Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                AlignedByteOffset: 4 * 2,
                ..Default::default()
            },
        ];

        let mut desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
            pRootSignature: unsafe { std::mem::transmute_copy(rs) },
            VS: ShaderBytecode::from(vs_dxil.as_slice()).into(),
            PS: ShaderBytecode::from(ps_dxil.as_slice()).into(),
            BlendState: BlendDesc::reasonable_default(),
            SampleMask: u32::MAX,
            RasterizerState: RasterizerDesc::reasonable_default(),
            InputLayout: D3D12_INPUT_LAYOUT_DESC {
                pInputElementDescs: input_layout.as_ptr(),
                NumElements: input_layout.len() as u32,
            },
            PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_POINT,
            NumRenderTargets: 1,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                ..Default::default()
            },
            ..Default::default()
        };
        desc.RTVFormats[0] = rtv_format;

        unsafe { device.CreateGraphicsPipelineState(&desc).unwrap() }
    }
}
