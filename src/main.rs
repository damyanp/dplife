use std::{error::Error, rc::Rc, sync::mpsc::{self, Receiver}, thread};

use d3dx12::transition_barrier;
use renderer::Renderer;
use ui::Ui;
use windows::Win32::{
    Foundation::HWND,
    Graphics::Direct3D12::{D3D12_RESOURCE_STATE_PRESENT, D3D12_RESOURCE_STATE_RENDER_TARGET},
};
use winit::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::EventLoop,
    platform::windows::WindowExtWindows,
    window::{Window, WindowBuilder},
};

mod renderer;
mod ui;

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

    let mut main_thread = Some(thread::spawn(move || { main_thread(rx) }));

    let mut renderer = Renderer::new(window.inner_size(), HWND(window.hwnd()));
    let mut ui = Ui::new(
        &window,
        &renderer.device,
        renderer.descriptor_heap.get_descriptor_handles(0),
    );

    event_loop.run(move |event, _, control_flow| {

        ui.handle_event(&window, &event);

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
            },
            Event::RedrawRequested(_) => render(&mut ui, &mut renderer, &window),
            _ => (),
        }
    });
}

fn main_thread(rx: Receiver<ThreadMessage>) {
    print!("main_thread!");
    'mainloop: loop {
        for message in rx.try_iter() {
            match message {                            
                ThreadMessage::Quit => break 'mainloop,
            }
        }
    }
    print!("leaving main_thread!");
}

fn render(ui: &mut Ui, renderer: &mut Renderer, _window: &Window) {
    // Prepare UI
    let imgui = ui.new_frame();
    imgui.show_demo_window(&mut true);

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

    ui.render(&cl);

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
