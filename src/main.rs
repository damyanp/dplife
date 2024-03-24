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
use renderer::{
    points::Vertex,
    Renderer,
};
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

    let (tx, rx) = mpsc::channel();

    let imgui_manager = Arc::new(ImguiManager::new(window));
    let imgui_manager_for_main_thread = imgui_manager.clone();

    let mut main_thread = Some(thread::spawn(move || {
        main_thread(rx, imgui_manager_for_main_thread)
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

#[derive(Default)]
struct UI {
    demo_window: bool,

    drag_start: Option<[f32; 2]>,
}

impl UI {
    pub fn render(&mut self, imgui: &mut imgui::Ui) {
        imgui
            .window("dplife")
            .position([5.0, 5.0], Always)
            .collapsed(true, imgui::Condition::Once)
            .build(|| {
                imgui.checkbox("Demo", &mut self.demo_window);

                if self.demo_window {
                    imgui.show_demo_window(&mut self.demo_window);
                }

                imgui.text(format!("wheel: {:?}", imgui.io().mouse_wheel));

                if !imgui.io().want_capture_mouse {
                    imgui.text(format!(
                        "Click: {:?} {:?}",
                        imgui.io().mouse_pos,
                        imgui.io().want_capture_mouse
                    ));

                    if imgui.io().mouse_down[0] {
                        if self.drag_start.is_none() {
                            self.drag_start = Some(imgui.io().mouse_pos);
                        }

                        if let Some(drag_start) = &self.drag_start {
                            imgui.text(format!(
                                "Drag {:?} -> {:?}",
                                drag_start,
                                imgui.io().mouse_pos
                            ));
                        }
                    }
                }
            });
    }
}

fn main_thread(rx: Receiver<ThreadMessage>, imgui_manager: Arc<Mutex<ImguiManager>>) {
    let mut im = imgui_manager.lock().unwrap();

    let mut renderer = Renderer::new(&im.window.lock().unwrap());
    let mut ui_renderer = im.new_renderer(
        &renderer.device,
        renderer.descriptor_heap.get_descriptor_handles(0),
    );

    drop(im);

    let mut ui = UI::default();

    let mut camera = Camera::new(renderer.get_viewport().clone());
    let mut points_renderer = renderer.new_points_renderer();

    let mut rng = thread_rng();
    let range = 0.0_f32..1024.0_f32;

    let verts: [Vertex; 1000] = array_init(|_| Vertex {
        position: [rng.gen_range(range.clone()), rng.gen_range(range.clone())],
        color: rng.gen_range(0..u32::MAX),
    });



    'mainloop: loop {
        #[allow(clippy::never_loop)]
        for message in rx.try_iter() {
            match message {
                ThreadMessage::Quit => break 'mainloop,
            }
        }

        renderer.start_new_frame();

        let render_target = renderer.get_render_target().clone();

        let cl = renderer.new_command_list();

        unsafe {
            cl.ResourceBarrier(&[transition_barrier(
                &render_target,
                D3D12_RESOURCE_STATE_PRESENT,
                D3D12_RESOURCE_STATE_RENDER_TARGET,
            )]);
        }

        renderer.set_viewports_and_scissors(&cl);

        unsafe {
            let rtv = renderer.get_rtv_handle();

            cl.OMSetRenderTargets(1, Some(&rtv), false, None);
            cl.ClearRenderTargetView(rtv, &[0.0_f32, 0.0_f32, 0.0_f32, 1.0_f32], None);
            cl.SetDescriptorHeaps(&[Some(renderer.descriptor_heap.heap.clone())]);
        }

        points_renderer.render(&camera, &cl, &verts);

        // Prepare UI
        {
            let mut imgui_manager = imgui_manager.lock().unwrap();

            let imgui = imgui_manager.new_frame(&mut ui_renderer);

            ui.render(imgui);

            camera.update(imgui);

            imgui_manager.render(&mut ui_renderer, &cl);
        }

        unsafe {
            cl.ResourceBarrier(&[transition_barrier(
                &render_target,
                D3D12_RESOURCE_STATE_RENDER_TARGET,
                D3D12_RESOURCE_STATE_PRESENT,
            )]);

            cl.Close().unwrap();
        }

        renderer.execute_command_lists(ecl![cl]);
        renderer.present();
        renderer.end_frame();
    }

    renderer.shutdown();
}
