use camera::Camera;
use particle_life::World;
use std::{
    error::Error,
    sync::{
        mpsc::{self, Receiver},
        Arc, Mutex,
    },
    thread,
};
use vek::Vec2;

use d3dx12::transition_barrier;
use imgui::Condition::Always;
use imgui_manager::ImguiManager;

use renderer::{
    points::{PointsRenderer, Vertex},
    Renderer,
};
use windows::Win32::Graphics::Direct3D12::{
    D3D12_RESOURCE_STATE_PRESENT, D3D12_RESOURCE_STATE_RENDER_TARGET,
};
use winit::{
    dpi::LogicalSize,
    event::{ElementState, Event, MouseScrollDelta, WindowEvent},
    event_loop::EventLoop,
    window::WindowBuilder,
};

mod camera;
mod imgui_manager;
mod particle_life;
mod renderer;

enum ThreadMessage<'a> {
    Quit,
    Event(Event<'a, ()>),
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
        let pass_event_to_app = imgui_manager.lock().unwrap().handle_event(&event);

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

        if main_thread.is_some() && pass_event_to_app {
            // event.to_static() consumes event, so we have to make this the
            // last thing we do with it. See
            // https://github.com/rust-windowing/winit/issues/1968.
            if let Some(static_event) = event.to_static() {
                tx.send(ThreadMessage::Event(static_event)).unwrap();
            }
        }
    });
}

struct RenderedUI {
    imgui_manager: Arc<Mutex<ImguiManager>>,
    imgui_renderer: imgui_windows_d3d12_renderer::Renderer,
}

#[derive(Default)]
struct UIState {
    new_rules: bool,
    scatter: bool,
}

impl UIState {
    fn draw_ui(&mut self, imgui: &mut imgui::Ui) {
        imgui
            .window("dplife")
            .position([5.0, 5.0], Always)
            .collapsed(true, imgui::Condition::Once)
            .build(|| {
                self.new_rules = imgui.button("New Rules");
                self.scatter = imgui.button("Scatter");
            });
    }
}

struct App {
    renderer: Renderer,
    points_renderer: PointsRenderer,
    camera: Camera,

    rendered_ui: RenderedUI,
    ui_state: UIState,

    world: World,
    world_rules: particle_life::Rules,

    verts: Vec<Vertex>,

    mouse: Mouse,
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
        let ui_state = UIState::default();

        let camera = Camera::new(*renderer.get_viewport());
        let points_renderer = renderer.new_points_renderer();

        const NUM_PARTICLES: usize = 2000;

        let verts = Vec::from_iter((0..NUM_PARTICLES).map(|_| Vertex {
            position: [0.0, 0.0],
            color: 0,
        }));

        let world_size = Vec2::new(
            renderer.get_viewport().Width,
            renderer.get_viewport().Height,
        );

        App {
            renderer,
            points_renderer,
            camera,
            rendered_ui,
            ui_state,
            world: World::new(NUM_PARTICLES, world_size),
            world_rules: particle_life::Rules::new_random(),
            verts,
            mouse: Mouse::new(),
        }
    }

    fn start_tick(&mut self) {
        self.mouse.start_tick();
    }

    fn update(&mut self) {
        if self.ui_state.new_rules {
            self.world_rules = particle_life::Rules::new_random();
        }

        if self.ui_state.scatter {
            self.world.scatter();
        }

        self.camera.update(&self.mouse);
        self.world.update(&self.world_rules, &mut self.verts);
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

            self.ui_state.draw_ui(imgui);

            self.mouse.draw_ui(imgui);

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

    fn handle_event<T>(&mut self, event: Event<'_, T>) {
        if let Event::WindowEvent {
            event: window_event,
            ..
        } = event
        {
            self.mouse.handle_event(window_event);
        }
    }
}

struct Mouse {
    position: Vec2<f32>,
    left_button: ElementState,
    right_button: ElementState,
    middle_button: ElementState,
    wheel: f32,
}

impl Mouse {
    fn new() -> Self {
        Mouse {
            position: Vec2::zero(),
            left_button: ElementState::Released,
            right_button: ElementState::Released,
            middle_button: ElementState::Released,
            wheel: 0.0,
        }
    }

    fn start_tick(&mut self) {
        self.wheel = 0.0;
    }

    fn handle_event(&mut self, window_event: WindowEvent<'_>) {
        match window_event {
            WindowEvent::CursorMoved { position, .. } => {
                self.position = Vec2::from((position.x as f32, position.y as f32));
            }
            WindowEvent::MouseInput { state, button, .. } => {
                use winit::event::MouseButton::*;

                match button {
                    Left => self.left_button = state,
                    Right => self.right_button = state,
                    Middle => self.middle_button = state,
                    _ => (),
                }
            }
            WindowEvent::MouseWheel {
                delta: MouseScrollDelta::LineDelta(_, delta_y),
                ..
            } => {
                self.wheel += delta_y;
            }

            _ => (),
        }
    }

    fn draw_ui(&self, _imgui: &mut imgui::Ui) {
        // _imgui.text(format!(
        //     "{:?} {:?} {:?} {:?} {:?}",
        //     self.position, self.left_button, self.right_button, self.middle_button, self.wheel
        // ));
    }
}

fn main_thread(
    rx: Receiver<ThreadMessage<'_>>,
    renderer: Renderer,
    imgui_manager: Arc<Mutex<ImguiManager>>,
) {
    let mut app = App::new(renderer, imgui_manager);

    'mainloop: loop {
        app.start_tick();

        #[allow(clippy::never_loop)]
        for message in rx.try_iter() {
            match message {
                ThreadMessage::Quit => break 'mainloop,
                ThreadMessage::Event(event) => app.handle_event(event),
            }
        }

        app.update();
        app.render();
    }
}
