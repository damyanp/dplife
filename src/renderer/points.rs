use std::{ffi::c_void, mem::size_of};

use array_init::array_init;
use windows::{
    core::s,
    Win32::Graphics::{
        Direct3D::D3D_PRIMITIVE_TOPOLOGY_POINTLIST,
        Direct3D12::{
            ID3D12Device, ID3D12GraphicsCommandList, ID3D12PipelineState, ID3D12Resource,
            ID3D12RootSignature, D3D12_GRAPHICS_PIPELINE_STATE_DESC, D3D12_HEAP_FLAG_NONE,
            D3D12_HEAP_TYPE_UPLOAD, D3D12_INPUT_ELEMENT_DESC, D3D12_INPUT_LAYOUT_DESC,
            D3D12_PRIMITIVE_TOPOLOGY_TYPE_POINT, D3D12_RESOURCE_STATE_VERTEX_AND_CONSTANT_BUFFER,
            D3D12_VERTEX_BUFFER_VIEW,
        },
        Dxgi::Common::{
            DXGI_FORMAT, DXGI_FORMAT_R32G32_FLOAT, DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_SAMPLE_DESC,
        },
    },
};

use d3dx12::{BlendDesc, HeapProperties, Mappable, RasterizerDesc, ResourceDesc, ShaderBytecode};

use crate::camera::Camera;

pub struct PointsRenderer {
    rs: ID3D12RootSignature,
    pso: ID3D12PipelineState,
}

pub struct PointsBuffers {
    vertex_buffers: [ID3D12Resource; 2],
    buffer_index: usize,
}

#[allow(dead_code)]
impl PointsBuffers {
    pub fn new(device: &ID3D12Device) -> Self {
        let vertex_buffers = Self::create_vertex_buffers(device);

        PointsBuffers {
            vertex_buffers,
            buffer_index: 0,
        }
    }

    fn create_vertex_buffers(device: &ID3D12Device) -> [ID3D12Resource; 2] {
        const INITIAL_VERTEX_COUNT: usize = 100000;

        array_init(|_| Self::create_vertex_buffer(device, INITIAL_VERTEX_COUNT))
    }

    fn create_vertex_buffer(device: &ID3D12Device, vertex_count: usize) -> ID3D12Resource {
        unsafe {
            let mut resource: Option<ID3D12Resource> = None;
            device
                .CreateCommittedResource(
                    &HeapProperties::standard(D3D12_HEAP_TYPE_UPLOAD),
                    D3D12_HEAP_FLAG_NONE,
                    &ResourceDesc::buffer(vertex_count * size_of::<Vertex>()),
                    D3D12_RESOURCE_STATE_VERTEX_AND_CONSTANT_BUFFER,
                    None,
                    &mut resource,
                )
                .unwrap();
            resource.unwrap()
        }
    }

    pub fn populate_next_buffer(&mut self, vertices: &[Vertex]) -> &ID3D12Resource {
        let num_buffers = self.vertex_buffers.len();
        let vertex_buffer = &mut self.vertex_buffers[self.buffer_index];

        let mut mapped = vertex_buffer.map();
        let slice = &mut mapped.as_mut_slice()[0..vertices.len()];
        slice.copy_from_slice(vertices);
        drop(mapped);

        self.buffer_index = (self.buffer_index + 1) % num_buffers;

        vertex_buffer
    }

    pub fn get_next_buffer(&mut self) -> &mut ID3D12Resource {
        let num_buffers = self.vertex_buffers.len();
        let vertex_buffer = &mut self.vertex_buffers[self.buffer_index];
        self.buffer_index = (self.buffer_index + 1) % num_buffers;
        vertex_buffer
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct Vertex {
    pub position: [f32; 2],
    pub color: u32,
}

//
// PointsRenderer construction
//
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

//
// PointsRenderer: render
//
impl PointsRenderer {
    pub fn render(
        &mut self,
        camera: &Camera,
        cl: &ID3D12GraphicsCommandList,
        vertex_buffer: &ID3D12Resource,
        num_points: u32,
    ) {
        unsafe {
            cl.SetGraphicsRootSignature(&self.rs);
            cl.SetPipelineState(&self.pso);

            let vbv = D3D12_VERTEX_BUFFER_VIEW {
                BufferLocation: vertex_buffer.GetGPUVirtualAddress(),
                SizeInBytes: vertex_buffer.GetDesc().Width as u32,
                StrideInBytes: size_of::<Vertex>() as u32,
            };

            cl.IASetVertexBuffers(0, Some(&[vbv]));
            cl.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_POINTLIST);

            let constant_buffer = camera.get_matrix().into_col_array();

            cl.SetGraphicsRoot32BitConstants(
                0,
                constant_buffer.len() as u32,
                std::ptr::addr_of!(constant_buffer) as *const c_void,
                0,
            );

            cl.DrawInstanced(num_points, 1, 0, 0);
        }
    }
}
