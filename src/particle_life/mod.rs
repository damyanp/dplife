use array_init::array_init;
use d3dx12::{HeapProperties, Mappable, ResourceDesc, ShaderBytecode};
use palette::{FromColor, Hsl, Srgb};
use rand::{thread_rng, Rng};
use std::{
    cell::RefCell,
    mem::{size_of, size_of_val, swap},
    ops::Range,
};
use vek::{num_traits::Euclid, Vec2};
use windows::Win32::Graphics::Direct3D12::{
    ID3D12Device, ID3D12GraphicsCommandList, ID3D12PipelineState, ID3D12Resource,
    ID3D12RootSignature, D3D12_COMPUTE_PIPELINE_STATE_DESC, D3D12_HEAP_FLAG_NONE, D3D12_HEAP_TYPE,
    D3D12_HEAP_TYPE_DEFAULT, D3D12_HEAP_TYPE_UPLOAD, D3D12_RESOURCE_STATE_COMMON,
};

use crate::renderer::points::Vertex;

pub struct World {
    shader_constants: ShaderGlobalConstants,
    staging_buffers: [ID3D12Resource; 2],
    reset_particles: bool,

    vertex_buffer: ID3D12Resource,
    old_particles: ID3D12Resource,
    new_particles: ID3D12Resource,
    constant_buffer: ID3D12Resource,

    rs: ID3D12RootSignature,
    pso: ID3D12PipelineState,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct ShaderGlobalConstants {
    particle_type_max: u32,
    num_particles: u32,
    world_size: [f32; 2],
}

impl World {
    pub fn new(device: &ID3D12Device, num_particles: usize, size: Vec2<f32>) -> Self {
        let shader_constants = ShaderGlobalConstants {
            particle_type_max: ParticleType::MAX as u32,
            num_particles: num_particles as u32,
            world_size: size.into_array(),
        };

        let particle_buffer_size = num_particles * size_of::<Particle>();
        let vertex_buffer_size = num_particles * size_of::<Vertex>();

        let num_rules = (ParticleType::MAX * ParticleType::MAX) as usize;
        let constant_buffer_size =
            size_of::<ShaderGlobalConstants>() + size_of::<Rule>() * num_rules;

        let rs = create_root_signature(device);
        let pso = create_pipeline_state(device, &rs);

        World {
            shader_constants,
            vertex_buffer: create_buffer(device, vertex_buffer_size),
            old_particles: create_buffer(device, particle_buffer_size),
            new_particles: create_buffer(device, particle_buffer_size),
            staging_buffers: array_init(|_| {
                create_upload_buffer(device, particle_buffer_size + constant_buffer_size)
            }),
            constant_buffer: create_buffer(device, constant_buffer_size),

            reset_particles: true,

            rs,
            pso,
        }
    }

    pub fn scatter(&mut self) {
        self.reset_particles = true;
    }

    pub fn update(&mut self, rules: &Rules, cl: &ID3D12GraphicsCommandList) {
        self.update_buffers(rules, cl);
        self.reset_particles = false;

        unsafe {
            cl.SetComputeRootSignature(&self.rs);
            cl.SetPipelineState(&self.pso);
            cl.SetComputeRootConstantBufferView(0, self.constant_buffer.GetGPUVirtualAddress());
            cl.SetComputeRootShaderResourceView(
                1,
                self.constant_buffer.GetGPUVirtualAddress()
                    + size_of::<ShaderGlobalConstants>() as u64,
            );
            cl.SetComputeRootShaderResourceView(2, self.old_particles.GetGPUVirtualAddress());
            cl.SetComputeRootUnorderedAccessView(3, self.new_particles.GetGPUVirtualAddress());
            cl.SetComputeRootUnorderedAccessView(4, self.vertex_buffer.GetGPUVirtualAddress());
            cl.Dispatch(self.shader_constants.num_particles / 32, 1, 1);
        }        

        swap(&mut self.old_particles, &mut self.new_particles);
    }

    fn update_buffers(&mut self, rules: &Rules, cl: &ID3D12GraphicsCommandList) {
        unsafe {
            let staging_dest = self.staging_buffers[0].clone();
            let staging = &mut self.staging_buffers[0];
            let mut dest = staging.map_raw();
            let mut dest_offset = 0;

            // Always copy the shader constants and rules
            *dest.as_mut_offset(dest_offset) = self.shader_constants;
            cl.CopyBufferRegion(
                &self.constant_buffer,
                dest_offset as u64,
                &staging_dest,
                dest_offset as u64,
                size_of_val(&self.shader_constants) as u64,
            );
            dest_offset += size_of_val(&self.shader_constants) as isize;

            *dest.as_mut_offset(dest_offset) = *rules;
            cl.CopyBufferRegion(
                &self.constant_buffer,
                dest_offset as u64,
                &staging_dest,
                dest_offset as u64,
                size_of_val(rules) as u64,
            );
            dest_offset += size_of_val(rules) as isize;

            // Copy a new set of random particles if needed
            if self.reset_particles {
                let size = Vec2::from(self.shader_constants.world_size);
                let num_particles = self.shader_constants.num_particles;

                let particles: Vec<_> = (0..num_particles).map(|_| Particle::new(&size)).collect();

                let dest_particles = dest.as_mut_slice_offset(dest_offset, num_particles as usize);
                dest_particles.copy_from_slice(particles.as_slice());

                cl.CopyBufferRegion(
                    &self.old_particles,
                    0,
                    &staging_dest,
                    dest_offset as u64,
                    num_particles as u64 * size_of::<Particle>() as u64,
                );
            }
        }
    }

    pub fn get_vertex_buffer(&self) -> (&ID3D12Resource, u32) {
        (&self.vertex_buffer, self.shader_constants.num_particles)
    }
}

fn create_buffer_with_type(
    device: &ID3D12Device,
    size: usize,
    heap_type: D3D12_HEAP_TYPE,
) -> ID3D12Resource {
    unsafe {
        let mut resource: Option<ID3D12Resource> = None;
        device
            .CreateCommittedResource(
                &HeapProperties::standard(heap_type),
                D3D12_HEAP_FLAG_NONE,
                &ResourceDesc::buffer(size),
                D3D12_RESOURCE_STATE_COMMON,
                None,
                &mut resource,
            )
            .unwrap();
        resource.unwrap()
    }
}

fn create_upload_buffer(device: &ID3D12Device, size: usize) -> ID3D12Resource {
    create_buffer_with_type(device, size, D3D12_HEAP_TYPE_UPLOAD)
}

fn create_buffer(device: &ID3D12Device, size: usize) -> ID3D12Resource {
    create_buffer_with_type(device, size, D3D12_HEAP_TYPE_DEFAULT)
}

fn generate_random_particles(num_particles: usize, size: Vec2<f32>) -> [RefCell<Vec<Particle>>; 2] {
    let particles: Vec<_> = (0..num_particles).map(|_| Particle::new(&size)).collect();
    [RefCell::new(particles.clone()), RefCell::new(particles)]
}

#[derive(Clone, Copy)]
#[repr(C)]
struct Particle {
    position: Vec2<f32>,
    velocity: Vec2<f32>,
    particle_type: ParticleType,
    force: Vec2<f32>,
}

impl Particle {
    fn new(size: &Vec2<f32>) -> Self {
        let x_coordinate_range = 0.0_f32..size.x;
        let y_coordinate_range = 0.0_f32..size.y;

        let mut rng = thread_rng();

        Particle {
            position: Vec2::new(
                rng.gen_range(x_coordinate_range.clone()),
                rng.gen_range(y_coordinate_range.clone()),
            ),
            velocity: Vec2::zero(),
            particle_type: ParticleType(rng.gen_range(0..ParticleType::MAX)),
            force: Vec2::zero(),
        }
    }

    fn update(&mut self, world_size: &Vec2<f32>, vertex: &mut Vertex) {
        self.velocity += self.force * 0.05;
        self.velocity *= 0.8;
        self.force = Vec2::zero();
        self.position += self.velocity;
        self.position = self.position.rem_euclid(world_size);

        *vertex = Vertex {
            position: self.position.into_array(),
            color: self.particle_type.as_color(),
        };
    }

    fn accumulate_force(
        &mut self,
        world_size: &Vec2<f32>,
        rule: &Rule,
        other_position: &Vec2<f32>,
    ) {
        let mut direction = other_position - self.position;

        // Handle wrapping
        if direction.x > world_size.x * 0.5 {
            direction.x -= world_size.x;
        }
        if direction.x < world_size.x * -0.5 {
            direction.x += world_size.x;
        }
        if direction.y > world_size.y * 0.5 {
            direction.y -= world_size.y;
        }
        if direction.y < world_size.y * -0.5 {
            direction.y += world_size.y;
        }

        let distance = direction.magnitude();
        let direction = direction.normalized();

        if distance < rule.min_distance {
            let repulsive_amount =
                rule.force.abs() * remap(distance, 0.0..rule.min_distance, 1.0..0.0) * -3.0;
            let repulsive = direction * repulsive_amount;
            self.force += repulsive;
        }

        if distance < rule.max_distance {
            let attract_amount = rule.force * remap(distance, 0.0..rule.max_distance, 1.0..0.0);
            let attract = direction * attract_amount;
            self.force += attract;
        }
    }
}

/// https://processing.org/reference/map_.html
fn remap(value: f32, current: Range<f32>, target: Range<f32>) -> f32 {
    let t = (value - current.start) / (current.end - current.start);

    target.start * (1.0 - t) + target.end * t
}

#[derive(Clone, Copy)]
struct ParticleType(u8);

impl ParticleType {
    const MAX: u8 = 8;

    fn as_color(&self) -> u32 {
        let hsl = Hsl::new_srgb(360.0 * (self.0 as f32 / Self::MAX as f32), 1.0, 0.5);
        let rgb = Srgb::from_color(hsl);
        rgb.into_format().into()
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Rules {
    rules: [Rule; (ParticleType::MAX * ParticleType::MAX) as usize],
}

impl Rules {
    pub fn new_random() -> Self {
        Rules {
            rules: array_init(|_| Rule::new_random()),
        }
    }

    fn get_rule(&self, a: ParticleType, b: ParticleType) -> &Rule {
        &self.rules[(a.0 * ParticleType::MAX + b.0) as usize]
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Rule {
    pub force: f32,
    pub min_distance: f32,
    pub max_distance: f32,
}

impl Rule {
    fn new_random() -> Self {
        let mut rng = thread_rng();

        let min_distance = rng.gen_range(30.0_f32..50.0_f32);
        let max_distance = min_distance + rng.gen_range(70.0_f32..250.0_f32);

        Rule {
            force: rng.gen_range(0.3_f32..1.0_f32) * if rng.gen_bool(0.5) { -1.0 } else { 1.0 },
            min_distance,
            max_distance,
        }
    }
}

fn create_root_signature(device: &ID3D12Device) -> ID3D12RootSignature {
    let rs = include_bytes!(concat!(env!("OUT_DIR"), "/particle_life.root_signature"));
    unsafe { device.CreateRootSignature(0, rs).unwrap() }
}

fn create_pipeline_state(device: &ID3D12Device, rs: &ID3D12RootSignature) -> ID3D12PipelineState {
    let dxil = include_bytes!(concat!(env!("OUT_DIR"), "/particle_life.dxil"));

    let desc = D3D12_COMPUTE_PIPELINE_STATE_DESC {
        pRootSignature: unsafe { std::mem::transmute_copy(rs) },
        CS: ShaderBytecode::from(dxil.as_slice()).into(),
        ..Default::default()
    };

    unsafe { device.CreateComputePipelineState(&desc).unwrap() }
}
