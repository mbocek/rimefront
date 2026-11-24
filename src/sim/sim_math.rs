use bevy::prelude::*;

// Rust's own `sin`, `atan2` and `powf` call the platform's maths library, whose last bit differs between
// processors and operating systems. Networked machines must compute the same battle, so the simulation
// uses the portable `libm` instead. Positions also stay in scalar `Vec3`, never in SIMD quaternions.

pub fn sin(angle: f32) -> f32 {
    libm::sinf(angle)
}

pub fn cos(angle: f32) -> f32 {
    libm::cosf(angle)
}

pub fn atan2(y: f32, x: f32) -> f32 {
    libm::atan2f(y, x)
}

pub fn powf(base: f32, exponent: f32) -> f32 {
    libm::powf(base, exponent)
}

/// The level direction something turned to `yaw` looks along; yaw 0 faces +X, as the models do.
pub fn facing_direction(yaw: f32) -> Vec3 {
    Vec3::new(cos(yaw), 0.0, -sin(yaw))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn facing_matches_turning_the_model_about_the_vertical() {
        for yaw in [0.0, 0.7, 2.1, -1.3] {
            let turned = Quat::from_rotation_y(yaw) * Vec3::X;
            assert!(facing_direction(yaw).distance(turned) < 1e-6, "yaw {yaw}");
        }
    }
}
