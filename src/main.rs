use std::{
    borrow::Borrow,
    error::Error,
    sync::{
        mpsc::{self, Receiver},
        Arc, Mutex,
    },
    thread,
};

use d3dx12::transition_barrier;
use imgui_manager::ImguiManager;
use renderer::Renderer;
use windows::Win32::{
    Foundation::HWND,
    Graphics::Direct3D12::{D3D12_RESOURCE_STATE_PRESENT, D3D12_RESOURCE_STATE_RENDER_TARGET},
};
use winit::{
    dpi::{LogicalSize, PhysicalSize},
    event::{Event, WindowEvent},
    event_loop::EventLoop,
    platform::windows::WindowExtWindows,
    window::WindowBuilder,
};

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

    let initial_size = window.inner_size();
    let hwnd = HWND(window.hwnd());

    let ui = Arc::new(ImguiManager::new(&window));
    let ui_for_main_thread = ui.clone();

    let mut main_thread = Some(thread::spawn(move || {
        main_thread(rx, initial_size, hwnd, ui_for_main_thread)
    }));

    event_loop.run(move |event, _, control_flow| {
        ui.lock().unwrap().handle_event(&window, &event);

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
}

impl UI {
    pub fn render(&mut self, imgui: &mut imgui::Ui) {
        imgui.checkbox("Demo", &mut self.demo_window);

        if self.demo_window {
            imgui.show_demo_window(&mut self.demo_window);
        }

        if !imgui.io().want_capture_mouse {
            imgui.text(format!("Click: {:?} {:?}", imgui.io().mouse_pos, imgui.io().want_capture_mouse));
        }
    }
}

fn main_thread(
    rx: Receiver<ThreadMessage>,
    initial_size: PhysicalSize<u32>,
    hwnd: HWND,
    imgui_manager: Arc<Mutex<ImguiManager>>,
) {
    let mut renderer = Renderer::new(initial_size, hwnd);
    let mut ui_renderer = imgui_manager.lock().unwrap().new_renderer(
        &renderer.device,
        renderer.descriptor_heap.get_descriptor_handles(0),
    );

    let mut ui = UI::default();

    'mainloop: loop {
        #[allow(clippy::never_loop)]
        for message in rx.try_iter() {
            match message {
                ThreadMessage::Quit => break 'mainloop,
            }
        }

        render(
            imgui_manager.borrow(),
            &mut renderer,
            &mut ui_renderer,
            &mut ui,
        );
    }

    renderer.shutdown();
}

fn render(
    imgui_manager: &Mutex<ImguiManager>,
    renderer: &mut Renderer,
    ui_renderer: &mut imgui_windows_d3d12_renderer::Renderer,
    ui: &mut UI,
) {
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
        cl.ClearRenderTargetView(rtv, &[0.0_f32, 0.2_f32, 0.4_f32, 1.0_f32], None);
        cl.SetDescriptorHeaps(&[Some(renderer.descriptor_heap.heap.clone())]);
    }

    // Prepare UI
    {
        let mut imgui_manager = imgui_manager.lock().unwrap();

        ui.render(imgui_manager.new_frame(ui_renderer));

        imgui_manager.render(ui_renderer, &cl);
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
