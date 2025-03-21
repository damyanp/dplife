use std::ffi::c_void;

use array_init::array_init;
use d3dx12::{CbvSrvUavDescriptorHeap, DescriptorHeap, RtvDescriptorHeap};
use windows::{
    core::{Interface, HSTRING},
    Win32::{
        Foundation::{HANDLE, HWND, RECT},
        Graphics::{
            Direct3D::D3D_FEATURE_LEVEL_11_0,
            Direct3D12::{
                D3D12CreateDevice, D3D12GetDebugInterface, ID3D12CommandAllocator,
                ID3D12CommandList, ID3D12CommandQueue, ID3D12Debug, ID3D12Device, ID3D12Fence,
                ID3D12GraphicsCommandList, ID3D12Resource, D3D12_COMMAND_LIST_TYPE_DIRECT,
                D3D12_COMMAND_QUEUE_DESC, D3D12_CPU_DESCRIPTOR_HANDLE,
                D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE, D3D12_FENCE_FLAG_NONE, D3D12_MAX_DEPTH,
                D3D12_MIN_DEPTH, D3D12_VIEWPORT,
            },
            Dxgi::{
                Common::{DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_SAMPLE_DESC},
                CreateDXGIFactory2, IDXGIAdapter1, IDXGIFactory4, IDXGISwapChain3,
                DXGI_CREATE_FACTORY_DEBUG, DXGI_MWA_NO_ALT_ENTER, DXGI_PRESENT,
                DXGI_SWAP_CHAIN_DESC1, DXGI_SWAP_EFFECT_FLIP_DISCARD,
                DXGI_USAGE_RENDER_TARGET_OUTPUT,
            },
        },
        System::Threading::{CreateEventA, WaitForSingleObject, INFINITE},
    },
};
use winit::window::{Window, WindowId};

pub mod points;

pub struct Renderer {
    pub device: ID3D12Device,
    command_queue: ID3D12CommandQueue,
    swap_chain: SwapChain,
    pub descriptor_heap: CbvSrvUavDescriptorHeap,
    frame_manager: Option<FrameManager>,
}

unsafe impl Send for Renderer {}

pub const FRAME_COUNT: usize = 2;

pub struct SwapChain {
    #[allow(clippy::struct_field_names)]
    swap_chain: IDXGISwapChain3,
    render_targets: [ID3D12Resource; FRAME_COUNT],
    rtv_heap: RtvDescriptorHeap,
    viewport: D3D12_VIEWPORT,
    scissor_rect: RECT,
}

pub struct FrameManager {
    fence: ID3D12Fence,
    fence_event: HANDLE,
    next_fence_value: u64,
    frame_index: usize,
    frames: [Frame; FRAME_COUNT],
}

pub struct Frame {
    _index: usize,
    fence_value: u64,
    command_allocator: ID3D12CommandAllocator,
    available_command_lists: Vec<ID3D12GraphicsCommandList>,
    used_command_lists: Vec<ID3D12GraphicsCommandList>,
    started: bool,
}

impl Renderer {
    pub fn new(window: &Window) -> Self {
        unsafe {
            let (factory, device) = create_device().unwrap();
            let command_queue = device
                .CreateCommandQueue(&D3D12_COMMAND_QUEUE_DESC {
                    Type: D3D12_COMMAND_LIST_TYPE_DIRECT,
                    ..Default::default()
                })
                .unwrap();

            let size = window.inner_size();
            let hwnd = HWND(std::mem::transmute::<WindowId, *mut c_void>(window.id()));

            let swap_chain = SwapChain::new(&factory, &device, &command_queue, size, hwnd);

            factory
                .MakeWindowAssociation(hwnd, DXGI_MWA_NO_ALT_ENTER)
                .unwrap();

            let descriptor_heap =
                CbvSrvUavDescriptorHeap::new(&device, 1, D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE)
                    .unwrap();

            let frame_manager = Some(FrameManager::new(&device));

            Renderer {
                device,
                command_queue,
                swap_chain,
                descriptor_heap,
                frame_manager,
            }
        }
    }

    pub fn start_new_frame(&mut self) {
        self.frame_manager.as_mut().unwrap().start_new_frame();
    }

    pub fn end_frame(&mut self) {
        self.frame_manager
            .as_mut()
            .unwrap()
            .end_frame(&self.command_queue);
    }

    pub fn new_command_list(&mut self) -> ID3D12GraphicsCommandList {
        let cl = self
            .frame_manager
            .as_mut()
            .unwrap()
            .new_command_list(&self.device);

        unsafe {
            cl.SetDescriptorHeaps(&[Some(self.descriptor_heap.heap.clone())]);
        }

        cl
    }

    pub fn set_viewports_and_scissors(&self, cl: &ID3D12GraphicsCommandList) {
        self.swap_chain.set_viewports_and_scissors(cl);
    }

    pub fn get_viewport(&self) -> &D3D12_VIEWPORT {
        self.swap_chain.get_viewport()
    }

    pub fn get_render_target(&self) -> &ID3D12Resource {
        self.swap_chain.get_render_target()
    }

    pub fn execute_command_lists(&self, cl: &[Option<ID3D12CommandList>]) {
        unsafe {
            self.command_queue.ExecuteCommandLists(cl);
        }
    }

    pub fn get_rtv_handle(&self) -> D3D12_CPU_DESCRIPTOR_HANDLE {
        self.swap_chain.get_rtv_handle()
    }

    pub fn present(&self) {
        self.swap_chain.present();
    }

    pub fn shutdown(&mut self) {
        // take away the frame_manager - effectively dropping the frame manager.
        // This will force all in-flight frames to complete.
        let frame_manager = self.frame_manager.take();
        drop(frame_manager);
    }

    pub fn new_points_renderer(&self) -> points::PointsRenderer {
        points::PointsRenderer::new(&self.device, DXGI_FORMAT_R8G8B8A8_UNORM)
    }
}

#[macro_export]
macro_rules! ecl {
    ( $( $x:expr ), * ) => {
        &[
            $(
                Some(windows::Win32::Graphics::Direct3D12::ID3D12CommandList::from(($x).to_owned())),
            )*
        ]
    }
}

impl SwapChain {
    fn new(
        factory: &IDXGIFactory4,
        device: &ID3D12Device,
        command_queue: &ID3D12CommandQueue,
        size: winit::dpi::PhysicalSize<u32>,
        hwnd: HWND,
    ) -> SwapChain {
        unsafe {
            let swap_chain: IDXGISwapChain3 = make_swap_chain(factory, command_queue, size, hwnd);

            let rtv_heap = RtvDescriptorHeap::new(device, FRAME_COUNT).unwrap();

            let render_targets = array_init(|i| {
                let render_target: ID3D12Resource =
                    swap_chain.GetBuffer(u32::try_from(i).unwrap()).unwrap();
                device.CreateRenderTargetView(
                    &render_target,
                    None,
                    rtv_heap.get_cpu_descriptor_handle(i),
                );
                render_target
                    .SetName(&HSTRING::from(format!("RenderTarget {i}")))
                    .unwrap();
                render_target
            });

            #[allow(clippy::cast_precision_loss)]
            let viewport = D3D12_VIEWPORT {
                TopLeftX: 0.0,
                TopLeftY: 0.0,
                Width: size.width as f32,
                Height: size.height as f32,
                MinDepth: D3D12_MIN_DEPTH,
                MaxDepth: D3D12_MAX_DEPTH,
            };

            let scissor_rect = RECT {
                left: 0,
                top: 0,
                right: size.width.try_into().unwrap(),
                bottom: size.height.try_into().unwrap(),
            };

            SwapChain {
                swap_chain,
                render_targets,
                rtv_heap,
                viewport,
                scissor_rect,
            }
        }
    }

    fn set_viewports_and_scissors(&self, cl: &ID3D12GraphicsCommandList) {
        unsafe {
            cl.RSSetViewports(&[self.viewport]);
            cl.RSSetScissorRects(&[self.scissor_rect]);
        }
    }

    fn get_viewport(&self) -> &D3D12_VIEWPORT {
        &self.viewport
    }

    fn get_render_target(&self) -> &ID3D12Resource {
        unsafe { &self.render_targets[self.swap_chain.GetCurrentBackBufferIndex() as usize] }
    }

    fn get_rtv_handle(&self) -> D3D12_CPU_DESCRIPTOR_HANDLE {
        unsafe {
            self.rtv_heap
                .get_cpu_descriptor_handle(self.swap_chain.GetCurrentBackBufferIndex() as usize)
        }
    }

    fn present(&self) {
        unsafe {
            self.swap_chain.Present(1, DXGI_PRESENT(0)).unwrap();
        }
    }
}

impl FrameManager {
    unsafe fn new(device: &ID3D12Device) -> Self {
        FrameManager {
            fence: device.CreateFence(0, D3D12_FENCE_FLAG_NONE).unwrap(),
            fence_event: CreateEventA(None, false, false, None).unwrap(),
            next_fence_value: 1,
            frame_index: 0,
            frames: array_init(|i| Frame::new(i, device)),
        }
    }

    fn start_new_frame(&mut self) {
        self.frame_index = (self.frame_index + 1) % FRAME_COUNT;

        let frame = &mut self.frames[self.frame_index];
        frame.start_new(&self.fence, self.fence_event);
    }

    fn end_frame(&mut self, command_queue: &ID3D12CommandQueue) {
        self.frames[self.frame_index].end(command_queue, &self.fence, self.next_fence_value);
        self.next_fence_value += 1;
    }

    fn new_command_list(&mut self, device: &ID3D12Device) -> ID3D12GraphicsCommandList {
        self.frames[self.frame_index].new_command_list(device)
    }
}

impl Drop for FrameManager {
    fn drop(&mut self) {
        unsafe {
            for frame in &self.frames {
                frame.wait(&self.fence, self.fence_event);
            }
        }
    }
}

impl Frame {
    unsafe fn new(index: usize, device: &ID3D12Device) -> Self {
        let command_allocator: ID3D12CommandAllocator = device
            .CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT)
            .unwrap();
        command_allocator
            .SetName(&HSTRING::from(format!("Command Allocator {index}")))
            .unwrap();

        Frame {
            _index: index,
            fence_value: 0,
            command_allocator,
            available_command_lists: Vec::new(),
            used_command_lists: Vec::new(),
            started: false,
        }
    }

    fn start_new(&mut self, fence: &ID3D12Fence, fence_event: HANDLE) {
        unsafe {
            assert!(!self.started);

            self.wait(fence, fence_event);

            self.command_allocator.Reset().unwrap();

            self.available_command_lists
                .append(&mut self.used_command_lists);

            self.started = true;
        }
    }

    fn end(
        &mut self,
        command_queue: &ID3D12CommandQueue,
        fence: &ID3D12Fence,
        next_fence_value: u64,
    ) {
        assert!(self.started);

        unsafe {
            command_queue.Signal(fence, next_fence_value).unwrap();
        }
        self.fence_value = next_fence_value;

        self.started = false;
    }

    unsafe fn wait(&self, fence: &ID3D12Fence, fence_event: HANDLE) {
        if self.fence_value == 0 {
            return;
        };
        fence
            .SetEventOnCompletion(self.fence_value, fence_event)
            .unwrap();
        WaitForSingleObject(fence_event, INFINITE);
    }

    fn new_command_list(&mut self, device: &ID3D12Device) -> ID3D12GraphicsCommandList {
        assert!(self.started);

        let command_list = if self.available_command_lists.is_empty() {
            unsafe {
                device
                    .CreateCommandList(
                        0,
                        D3D12_COMMAND_LIST_TYPE_DIRECT,
                        &self.command_allocator,
                        None,
                    )
                    .unwrap()
            }
        } else {
            let command_list = self.available_command_lists.pop().unwrap();
            unsafe {
                command_list.Reset(&self.command_allocator, None).unwrap();
            }
            command_list
        };

        self.used_command_lists.push(command_list.clone());
        command_list
    }
}

unsafe fn make_swap_chain(
    factory: &IDXGIFactory4,
    command_queue: &ID3D12CommandQueue,
    size: winit::dpi::PhysicalSize<u32>,
    hwnd: HWND,
) -> IDXGISwapChain3 {
    let swap_chain_desc = DXGI_SWAP_CHAIN_DESC1 {
        BufferCount: u32::try_from(FRAME_COUNT).unwrap(),
        Width: size.width,
        Height: size.height,
        Format: DXGI_FORMAT_R8G8B8A8_UNORM,
        BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
        SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            ..Default::default()
        },
        ..Default::default()
    };

    factory
        .CreateSwapChainForHwnd(command_queue, hwnd, &swap_chain_desc, None, None)
        .unwrap()
        .cast()
        .unwrap()
}

fn create_device() -> windows::core::Result<(IDXGIFactory4, ID3D12Device)> {
    unsafe {
        let mut debug: Option<ID3D12Debug> = None;
        if let Some(debug) = D3D12GetDebugInterface(&mut debug).ok().and(debug) {
            debug.EnableDebugLayer();
        }
    }

    let dxgi_factory_flags = DXGI_CREATE_FACTORY_DEBUG;
    let dxgi_factory: IDXGIFactory4 = unsafe { CreateDXGIFactory2(dxgi_factory_flags) }?;

    let adapter = get_hardware_adapter(&dxgi_factory)?;

    let mut device: Option<ID3D12Device> = None;
    unsafe { D3D12CreateDevice(&adapter, D3D_FEATURE_LEVEL_11_0, &mut device) }?;
    Ok((dxgi_factory, device.unwrap()))
}

fn get_hardware_adapter(factory: &IDXGIFactory4) -> windows::core::Result<IDXGIAdapter1> {
    for i in 0.. {
        let adapter = unsafe { factory.EnumAdapters1(i)? };

        // let desc = unsafe { adapter.GetDesc1()? };
        //
        // if (DXGI_ADAPTER_FLAG(desc.Flags as i32) & DXGI_ADAPTER_FLAG_SOFTWARE)
        //     != DXGI_ADAPTER_FLAG_NONE
        // {
        //     // Don't select the Basic Render Driver adapter. If you want a
        //     // software adapter, pass in "/warp" on the command line.
        //     continue;
        // }

        // Check to see whether the adapter supports Direct3D 12, but don't
        // create the actual device yet.
        if unsafe {
            D3D12CreateDevice(
                &adapter,
                D3D_FEATURE_LEVEL_11_0,
                std::ptr::null_mut::<Option<ID3D12Device>>(),
            )
        }
        .is_ok()
        {
            return Ok(adapter);
        }
    }

    unreachable!()
}
