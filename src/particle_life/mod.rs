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
    buffer_index: usize,
    particles: [RefCell<Vec<Particle>>; 2],
}

impl World {
    pub fn new(num_particles: usize) -> Self {
        let particles: Vec<_> = (0..num_particles).map(|_| Particle::new()).collect();

        let particles = [RefCell::new(particles.clone()), RefCell::new(particles)];

        World {
            buffer_index: 0,
            particles,
        }
    }

    pub fn update(&mut self, rules: &Rules, vertices: &mut [Vertex]) {
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
                    rules.get_rule(old_a.particle_type, old_b.particle_type),
                    &old_b.position,
                );

                pout[index_b].accumulate_force(
                    rules.get_rule(old_b.particle_type, old_a.particle_type),
                    &old_a.position,
                );
            }
        }

        for (particle, vertex) in zip(pout.iter_mut(), vertices) {
            particle.update(vertex);
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

#[derive(Clone)]
struct Particle {
    position: Vec2<f32>,
    velocity: Vec2<f32>,
    particle_type: ParticleType,
    force: Vec2<f32>,
}

impl Particle {
    fn new() -> Self {
        let coordinate_range = 0.0_f32..1024.0_f32;
        let mut rng = thread_rng();

        Particle {
            position: Vec2::new(
                rng.gen_range(coordinate_range.clone()),
                rng.gen_range(coordinate_range.clone()),
            ),
            velocity: Vec2::zero(),
            particle_type: ParticleType(rng.gen_range(0..ParticleType::MAX)),
            force: Vec2::zero(),
        }
    }

    fn update(&mut self, vertex: &mut Vertex) {
        self.velocity += self.force;
        self.force = Vec2::zero();
        self.position += self.velocity;
        self.position = self.position.rem_euclid(&Vec2::new(1024.0, 1024.0));

        *vertex = Vertex {
            position: self.position.into_array(),
            color: self.particle_type.as_color(),
        };
    }

    fn accumulate_force(&mut self, rule: &Rule, other_position: &Vec2<f32>) {
        let mut direction = other_position - self.position;

        let width = 1024.0;
        let height = 1024.0;

        // Handle wrapping
        if direction.x > width * 0.5 {
            direction.x -= width;
        }
        if direction.x < width * -0.5 {
            direction.x += width;
        }
        if direction.y > height * 0.5 {
            direction.y -= height;
        }
        if direction.y < height * -0.5 {
            direction.y += height;
        }

        let distance = direction.magnitude();
        let direction = direction.normalized();

        if distance < rule.min_distance {
            let repulsive_amount =
                rule.force.abs() * remap(distance, 0.0..rule.min_distance, 1.0..0.0) * -3.0;
            let repulsive = direction * repulsive_amount * FORCE_SCALE;
            self.force += repulsive;
        }

        if distance < rule.max_distance {
            let attract_amount = rule.force * remap(distance, 0.0..rule.max_distance, 1.0..0.0);
            let attract = direction * attract_amount * FORCE_SCALE;
            self.force += attract;
        }
    }
}

const FORCE_SCALE: f32 = 0.001;

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

        let min_distance = rng.gen_range(0.0_f32..50.0_f32);
        let max_distance = min_distance + rng.gen_range(0.0_f32..200.0_f32);

        Rule {
            force: rng.gen_range(-1.0_f32..1.0_f32),
            min_distance,
            max_distance,
        }
    }
}
