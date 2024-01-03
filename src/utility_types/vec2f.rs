use std::ops;

type Component = f32;

// A 0-1 normalized floating-point vec2
#[derive(Copy, Clone)]
pub struct Vec2f {
	x: Component,
	y: Component
}

// TODO: make associated constants for the zero and one vectors

//////////

pub fn assert_in_unit_interval(f: Component) {
	std::assert!((0.0..=1.0).contains(&f));
}

//////////

impl Vec2f {
	pub fn new_from_one(f: Component) -> Self {
		assert_in_unit_interval(f);
		Self {x: f, y: f}
	}

	pub fn new(x: Component, y: Component) -> Self {
		assert_in_unit_interval(x);
		assert_in_unit_interval(y);
		Self {x, y}
	}

	pub fn x(&self) -> Component {
		self.x
	}

	pub fn y(&self) -> Component {
		self.y
	}

	pub fn translate_x(&self, x: Component) -> Self {
		Vec2f::new(self.x + x, self.y)
	}

	pub fn translate_y(&self, y: Component) -> Self {
		Vec2f::new(self.x, self.y + y)
	}
}

/* TODO:
- Automatically derive these
- Perhaps clamp the outputs instead
*/

impl ops::Add for Vec2f {
	type Output = Self;

	fn add(self, other: Self) -> Self::Output {
		Self::new(self.x + other.x, self.y + other.y)
	}
}

impl ops::Sub for Vec2f {
	type Output = Self;

	fn sub(self, other: Self) -> Self::Output {
		Self::new(self.x - other.x, self.y - other.y)
	}
}

impl ops::Mul for Vec2f {
	type Output = Self;

	fn mul(self, other: Self) -> Self::Output {
		Self::new(self.x * other.x(), self.y * other.y())
	}
}

impl ops::MulAssign<Vec2f> for Vec2f {
	fn mul_assign(&mut self, v: Self) {
		self.x *= v.x();
		assert_in_unit_interval(self.x);

		self.y *= v.y();
		assert_in_unit_interval(self.y);
	}
}
