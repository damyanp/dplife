use array_init::array_init;
use palette::{FromColor, Hsl, Srgb};
use rand::{thread_rng, Rng};
use std::{
    cell::{Ref, RefCell, RefMut},
    iter::zip,
    ops::Range,
};
use vek::{num_traits::Euclid, Vec2};

use crate::renderer::points::Vertex;

pub struct World {
    size: Vec2<f32>,
    buffer_index: usize,
    particles: [RefCell<Vec<Particle>>; 2],
}

impl World {
    pub fn new(num_particles: usize, size: Vec2<f32>) -> Self {
        let particles = generate_random_particles(num_particles, size);

        World {
            size,
            buffer_index: 0,
            particles,
        }
    }

    pub fn scatter(&mut self) {
        let count = self.particles[0].borrow().len();

        self.particles = generate_random_particles(count, self.size);
    }

    pub fn update(&mut self, rules: &Rules, vertices: &mut [Vertex]) {
        let size = self.size;
        let (pin, mut pout) = self.get_particles();

        // Prepare for update
        for (old, new) in zip(pin.iter(), pout.iter_mut()) {
            *new = old.clone();
        }

        // Collect forces
        for index_a in 0..pin.len() {
            for index_b in index_a + 1..pin.len() {
                assert!(index_a != index_b);

                let old_a = &pin[index_a];
                let old_b = &pin[index_b];

                pout[index_a].accumulate_force(
                    &size,
                    rules.get_rule(old_a.particle_type, old_b.particle_type),
                    &old_b.position,
                );

                pout[index_b].accumulate_force(
                    &size,
                    rules.get_rule(old_b.particle_type, old_a.particle_type),
                    &old_a.position,
                );
            }
        }

        for (particle, vertex) in zip(pout.iter_mut(), vertices) {
            particle.update(&size, vertex);
        }

        drop(pin);
        drop(pout);

        self.buffer_index += 1;
    }

    fn get_particles(&mut self) -> (Ref<'_, Vec<Particle>>, RefMut<'_, Vec<Particle>>) {
        let in_index = self.buffer_index % 2;
        let out_index = (self.buffer_index + 1) % 2;

        (
            self.particles[in_index].borrow(),
            self.particles[out_index].borrow_mut(),
        )
    }
    
}

fn generate_random_particles(num_particles: usize, size: Vec2<f32>) -> [RefCell<Vec<Particle>>; 2] {
    let particles: Vec<_> = (0..num_particles).map(|_| Particle::new(&size)).collect();
    [RefCell::new(particles.clone()), RefCell::new(particles)]
}

#[derive(Clone)]
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

    fn accumulate_force(&mut self, world_size: &Vec2<f32>, rule: &Rule, other_position: &Vec2<f32>) {
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
