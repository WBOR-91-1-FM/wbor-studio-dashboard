// A 0-1 normalized floating-point vec2
#[derive(Copy, Clone)]
pub struct Vec2f {
	x: f32,
	y: f32
}

pub fn assert_in_unit_interval(f: f32) {
	std::assert!(f >= 0.0 && f <= 1.0);
}

impl Vec2f {
	pub fn new(x: f32, y: f32) -> Self {
		assert_in_unit_interval(x);
		assert_in_unit_interval(y);
		Self {x, y}
	}

	pub fn x(&self) -> f32 {
		self.x
	}

	pub fn y(&self) -> f32 {
		self.y
	}

	pub fn translate_x(&self, x: f32) -> Self {
		Vec2f::new(self.x + x, self.y)
	}

	pub fn translate_y(&self, y: f32) -> Self {
		Vec2f::new(self.x, self.y + y)
	}
}

/* TODO:
- Automatically derive these
- Perhaps clamp the outputs instead
*/

impl std::ops::Add for Vec2f {
	type Output = Self;

	fn add(self, other: Self) -> Self::Output {
		Self::new(self.x + other.x, self.y + other.y)
	}
}


impl std::ops::Sub for Vec2f {
	type Output = Self;

	fn sub(self, other: Self) -> Self::Output {
		Self::new(self.x - other.x, self.y - other.y)
	}
}
