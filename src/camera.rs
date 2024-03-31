use vek::{num_traits::Zero, Mat4, Vec2};
use windows::Win32::Graphics::Direct3D12::D3D12_VIEWPORT;
use winit::event::ElementState;

use crate::Mouse;

type Point = Vec2<f32>;

pub struct Camera {
    viewport: D3D12_VIEWPORT,
    pos: Point,
    zoom: f32, // eg 1 = scale 1, 2 = scale 2, 3 = scale 4, -1 = scale 0.5 etc.
    scale: f32,
    matrix: Mat4<f32>,

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
            matrix: Self::calculate_matrix(pos, scale),
            move_operation: None,
        }
    }

    pub fn update(&mut self, mouse: &Mouse) {
        let mouse_pos = self.window_to_view(mouse.position);

        if mouse.middle_button == ElementState::Pressed {
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

        if !mouse.wheel.is_zero() {
            self.zoom += mouse.wheel / 10.0;

            // When zoom is < -11 things go bad. Needs debugging fully. Maybe
            // precision problems for really small numbers?
            self.zoom = self.zoom.max(-11.0); //

            self.scale = 2.0_f32.powf(self.zoom);

            let m = Self::calculate_matrix(self.pos, self.scale).inverted_affine_transform();
            let new_world_mouse_pos = Point::from(m * vek::Vec4::from_point(mouse_pos));

            let delta = new_world_mouse_pos - world_mouse_pos;

            self.pos += delta;
        }

        let dest_matrix = Self::calculate_matrix(self.pos, self.scale);
        self.matrix = self.matrix.map(|e| e * 0.6) + dest_matrix.map(|e| e * 0.4);
    }

    pub fn get_matrix(&self) -> Mat4<f32> {
        self.matrix
    }

    fn calculate_matrix(pos:Vec2<f32>, scale: f32) -> Mat4<f32> {
        let translate: Mat4<f32> = Mat4::translation_2d(pos);
        let scale = Mat4::scaling_3d(scale);

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

        window_pos / Vec2::new(self.viewport.Width / 2.0, self.viewport.Height / 2.0)
    }

    pub fn view_to_world(&self, view_pos: Point) -> Point {
        let m = self.get_matrix().inverted_affine_transform();
        Point::from(m * vek::Vec4::from_point(view_pos))
    }
}
