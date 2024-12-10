use std::ops;

/* TODO: could I wrap this in a struct type?
That would then allow for things like closure under multiplication */
type Component = f64;

// A 0-1 normalized floating-point vec2
#[derive(Copy, Clone, PartialEq)]
pub struct Vec2f {
	x: Component,
	y: Component
}

//////////

pub fn assert_in_unit_interval(f: Component) {
	assert!((0.0..=1.0).contains(&f));
}

//////////

impl Vec2f {
	pub const ZERO: Self = Self {x: 0.0, y: 0.0};
	pub const ONE: Self = Self {x: 1.0, y: 1.0};

	pub fn new_scalar(f: Component) -> Self {
		assert_in_unit_interval(f);
		Self {x: f, y: f}
	}

	pub fn new(x: Component, y: Component) -> Self {
		assert_in_unit_interval(x);
		assert_in_unit_interval(y);
		Self {x, y}
	}

	pub const fn x(&self) -> Component {
		self.x
	}

	pub const fn y(&self) -> Component {
		self.y
	}

	pub fn translate_x(&self, x: Component) -> Self {
		Vec2f::new(self.x + x, self.y)
	}

	pub fn translate_y(&self, y: Component) -> Self {
		Vec2f::new(self.x, self.y + y)
	}

	pub fn translate(&self, x: Component, y: Component) -> Self {
		Vec2f::new(self.x + x, self.y + y)
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
