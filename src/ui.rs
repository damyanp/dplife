use std::{sync::Mutex, time::Instant};

use d3dx12::DescriptorHandles;
use imgui::{FontConfig, FontSource};
use imgui_windows_d3d12_renderer::Renderer;
use imgui_winit_support::{HiDpiMode, WinitPlatform};
use windows::Win32::Graphics::{
    Direct3D12::{ID3D12Device, ID3D12GraphicsCommandList},
    Dxgi::Common::DXGI_FORMAT_R8G8B8A8_UNORM,
};
use winit::{
    event::{Event, WindowEvent},
    window::Window,
};

pub struct Ui {
    imgui: imgui::Context,
    winit_platform: WinitPlatform,
    last_frame_instant: Instant,
}

unsafe impl Send for Ui {}

impl Ui {
    pub fn new(window: &Window) -> Mutex<Self> {
        let mut imgui = imgui::Context::create();
        let mut winit_platform = WinitPlatform::init(&mut imgui);

        winit_platform.attach_window(imgui.io_mut(), window, HiDpiMode::Rounded);

        let hidpi_factor = winit_platform.hidpi_factor();
        let font_size = (13.0 * hidpi_factor) as f32;
        imgui.fonts().add_font(&[FontSource::DefaultFontData {
            config: Some(FontConfig {
                size_pixels: font_size,
                ..FontConfig::default()
            }),
        }]);

        imgui.io_mut().font_global_scale = (1.0 / hidpi_factor) as f32;

        Mutex::new(Ui {
            imgui,
            winit_platform,
            last_frame_instant: Instant::now(),
        })
    }

    pub fn get_renderer(
        &mut self,
        device: &ID3D12Device,
        font_descriptor_handles: DescriptorHandles,
    ) -> Renderer {
        Renderer::new(
            &mut self.imgui,
            device.clone(),
            crate::renderer::FRAME_COUNT,
            DXGI_FORMAT_R8G8B8A8_UNORM,
            font_descriptor_handles.cpu,
            font_descriptor_handles.gpu,
        )
        .unwrap()
    }

    pub fn handle_event(&mut self, window: &Window, event: &Event<'_, ()>) {
        match event {
            Event::NewEvents(_) => {
                let now = Instant::now();
                self.imgui
                    .io_mut()
                    .update_delta_time(now - self.last_frame_instant);
                self.last_frame_instant = now;
            }
            Event::MainEventsCleared => {
                self.winit_platform
                    .prepare_frame(self.imgui.io_mut(), window)
                    .unwrap();
                window.request_redraw();
            }
            Event::WindowEvent {
                event: WindowEvent::Resized(_),
                ..
            } => self
                .winit_platform
                .handle_event(self.imgui.io_mut(), window, event),

            event => self
                .winit_platform
                .handle_event(self.imgui.io_mut(), window, event),
        }
    }

    pub fn new_frame(&mut self, renderer: &mut Renderer) -> &mut imgui::Ui {
        renderer.new_frame(&mut self.imgui).unwrap();
        self.imgui.new_frame()
    }

    pub fn render(&mut self, renderer: &mut Renderer, cl: &ID3D12GraphicsCommandList) {
        renderer.render_draw_data(self.imgui.render(), cl);
    }
}
