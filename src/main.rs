use array_init::array_init;
use camera::Camera;
use std::{
    error::Error,
    sync::{
        mpsc::{self, Receiver},
        Arc, Mutex,
    },
    thread,
};

use d3dx12::transition_barrier;
use imgui::Condition::Always;
use imgui_manager::ImguiManager;
use rand::{thread_rng, Rng};
use renderer::{points::{PointsRenderer, Vertex}, Renderer};
use windows::Win32::Graphics::Direct3D12::{
    D3D12_RESOURCE_STATE_PRESENT, D3D12_RESOURCE_STATE_RENDER_TARGET,
};
use winit::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::EventLoop,
    window::WindowBuilder,
};

mod camera;
mod imgui_manager;
mod renderer;

enum ThreadMessage {
    Quit,
}

fn main() -> Result<(), Box<dyn Error>> {
    let event_loop = EventLoop::new();

    let builder = WindowBuilder::new().with_inner_size(LogicalSize {
        width: 1024,
        height: 768,
    });

    let window = builder.build(&event_loop)?;

    let renderer = Renderer::new(&window);

    let (tx, rx) = mpsc::channel();

    let imgui_manager = Arc::new(ImguiManager::new(window));
    let imgui_manager_for_main_thread = imgui_manager.clone();

    let mut main_thread = Some(thread::spawn(move || {
        main_thread(rx, renderer, imgui_manager_for_main_thread)
    }));

    event_loop.run(move |event, _, control_flow| {
        imgui_manager.lock().unwrap().handle_event(&event);

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                if let Some(thread) = main_thread.take() {
                    tx.send(ThreadMessage::Quit).unwrap();
                    thread.join().unwrap();
                }
                control_flow.set_exit();
            }
            Event::RedrawRequested(_) => (),
            _ => (),
        }
    });
}

struct RenderedUI {
    imgui_manager: Arc<Mutex<ImguiManager>>,
    imgui_renderer: imgui_windows_d3d12_renderer::Renderer,
}

struct UIState {
    demo_window: bool,
}

impl UIState {
    fn render(&mut self, imgui: &mut imgui::Ui) {
        imgui
            .window("dplife")
            .position([5.0, 5.0], Always)
            .collapsed(true, imgui::Condition::Once)
            .build(|| {
                imgui.checkbox("Demo", &mut self.demo_window);

                if self.demo_window {
                    imgui.show_demo_window(&mut self.demo_window);
                }
            });
    }
}

struct App {
    renderer: Renderer,
    points_renderer: PointsRenderer,
    camera: Camera,

    rendered_ui: RenderedUI,
    ui_state: UIState,

    verts: [Vertex; 1000],
}

impl Drop for App {
    fn drop(&mut self) {
        self.renderer.shutdown();
    }
}

impl App {
    fn new(renderer: Renderer, imgui_manager: Arc<Mutex<ImguiManager>>) -> Self {

        let mut im = imgui_manager.lock().unwrap();

        let imgui_renderer = im.new_renderer(
            &renderer.device,
            renderer.descriptor_heap.get_descriptor_handles(0),
        );
    
        drop(im);
    
        let rendered_ui = RenderedUI {
            imgui_manager,
            imgui_renderer,
        };
        let ui_state = UIState { demo_window: false };
    
        let camera = Camera::new(renderer.get_viewport().clone());
        let points_renderer = renderer.new_points_renderer();
    
        let mut rng = thread_rng();
        let range = 0.0_f32..1024.0_f32;
    
        let verts: [Vertex; 1000] = array_init(|_| Vertex {
            position: [rng.gen_range(range.clone()), rng.gen_range(range.clone())],
            color: rng.gen_range(0..u32::MAX),
        });
    
        App {
            renderer,
            points_renderer,
            camera,
            rendered_ui,
            ui_state,
            verts,
        }
    }

    fn update(&mut self) {
    }

    fn render(&mut self) {
        self.renderer.start_new_frame();

        let render_target = self.renderer.get_render_target().clone();

        let cl = self.renderer.new_command_list();

        unsafe {
            cl.ResourceBarrier(&[transition_barrier(
                &render_target,
                D3D12_RESOURCE_STATE_PRESENT,
                D3D12_RESOURCE_STATE_RENDER_TARGET,
            )]);
        }

        self.renderer.set_viewports_and_scissors(&cl);

        unsafe {
            let rtv = self.renderer.get_rtv_handle();

            cl.OMSetRenderTargets(1, Some(&rtv), false, None);
            cl.ClearRenderTargetView(rtv, &[0.0_f32, 0.0_f32, 0.0_f32, 1.0_f32], None);
        }

        self.points_renderer.render(&self.camera, &cl, &self.verts);

        // Prepare UI
        {
            let mut imgui_manager = self.rendered_ui.imgui_manager.lock().unwrap();

            let imgui = imgui_manager.new_frame(&mut self.rendered_ui.imgui_renderer);

            self.ui_state.render(imgui);

            self.camera.update(imgui.io());

            imgui_manager.render(&mut self.rendered_ui.imgui_renderer, &cl);
        }

        unsafe {
            cl.ResourceBarrier(&[transition_barrier(
                &render_target,
                D3D12_RESOURCE_STATE_RENDER_TARGET,
                D3D12_RESOURCE_STATE_PRESENT,
            )]);

            cl.Close().unwrap();
        }

        self.renderer.execute_command_lists(ecl![cl]);
        self.renderer.present();
        self.renderer.end_frame();
    }
}

fn main_thread(
    rx: Receiver<ThreadMessage>,
    renderer: Renderer,
    imgui_manager: Arc<Mutex<ImguiManager>>,
) {
    let mut app = App::new(renderer, imgui_manager);

    'mainloop: loop {
        #[allow(clippy::never_loop)]
        for message in rx.try_iter() {
            match message {
                ThreadMessage::Quit => break 'mainloop,
            }
        }

        app.update();
        app.render();

    }
}
