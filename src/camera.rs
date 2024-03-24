use vek::{num_traits::Zero, Mat4, Vec2};
use windows::Win32::Graphics::Direct3D12::D3D12_VIEWPORT;

type Point = Vec2<f32>;

pub struct Camera {
    viewport: D3D12_VIEWPORT,
    pos: Point,
    zoom: f32, // eg 1 = scale 1, 2 = scale 2, 3 = scale 4, -1 = scale 0.5 etc.
    scale: f32,

    move_operation: Option<MoveOperation>,
}

struct MoveOperation {
    mouse_start: Point,
    pos_start: Point,
}

impl Camera {
    pub fn new(viewport: D3D12_VIEWPORT) -> Self {
        let pos = Point::new(
            -viewport.Width / 2.0 - viewport.TopLeftX,
            -viewport.Height / 2.0 - viewport.TopLeftY,
        );

        let scale = 1.0 / (viewport.Width / 2.0);
        let zoom = scale.log2();

        Camera {
            viewport,
            pos,
            zoom,
            scale,
            move_operation: None,
        }
    }

    pub fn update(&mut self, io: &imgui::Io) {
        let mouse_pos = self.window_to_view(Point::from_slice(&io.mouse_pos));

        if io[imgui::MouseButton::Middle] {
            if let Some(move_operation) = &self.move_operation {
                let delta = mouse_pos - move_operation.mouse_start;
                self.pos = move_operation.pos_start + delta / self.scale;
            } else {
                self.move_operation = Some(MoveOperation {
                    mouse_start: mouse_pos,
                    pos_start: self.pos,
                });
            }
        } else {
            self.move_operation = None;
        }

        let world_mouse_pos = self.view_to_world(mouse_pos);

        if !io.mouse_wheel.is_zero() {
            self.zoom += io.mouse_wheel / 10.0;

            // When zoom is < -11 things go bad. Needs debugging fully. Maybe
            // precision problems for really small numbers?
            self.zoom = self.zoom.max(-11.0); // 
            
            self.scale = 2.0_f32.powf(self.zoom);

            let new_world_mouse_pos = self.view_to_world(mouse_pos);

            let delta= new_world_mouse_pos - world_mouse_pos;

            self.pos += delta;
        }
    }

    pub fn get_matrix(&self) -> Mat4<f32> {
        let translate: Mat4<f32> = Mat4::translation_2d(self.pos);
        let scale = Mat4::scaling_3d(self.scale);

        scale * translate
    }

    pub fn window_to_view(&self, window_pos: Point) -> Point {
        let window_pos = Point {
            x: window_pos.x,
            y: self.viewport.Height - window_pos.y,
        };
        // (0,0) is the center of the viewport
        let top_left = Vec2::new(self.viewport.TopLeftX, self.viewport.TopLeftY);
        let bottom_right = top_left + Vec2::new(self.viewport.Width, self.viewport.Height);
        let center = (top_left + bottom_right) / 2.0;
        let window_pos = window_pos - center;
        let window_pos = window_pos / Vec2::new(self.viewport.Width / 2.0, self.viewport.Height / 2.0);

        window_pos
    }

    pub fn view_to_world(&self, view_pos: Point) -> Point {
        let m = self.get_matrix().inverted_affine_transform();
        Point::from(m * vek::Vec4::from_point(view_pos))
    }
}
