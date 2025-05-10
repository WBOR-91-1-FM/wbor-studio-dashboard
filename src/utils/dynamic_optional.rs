//////////

pub struct DynamicOptional {
	inner: Option<Box<dyn std::any::Any>>
}

impl DynamicOptional {
	pub const NONE: Self = Self {inner: None};

	pub fn new<T: 'static>(value: T) -> DynamicOptional {
		DynamicOptional {inner: Some(Box::new(value))}
	}

	////////// TODO: eliminate the repetition here

	fn fail_for_inner_access<T>() -> ! {
		panic!(
			"Could not access the inner value of a DynamicOptional, given the expected type '{}'",
			std::any::type_name::<T>()
		);
	}

	pub fn get<T: 'static>(&self) -> &T {
		if let Some(boxed_inner_value) = &self.inner {
			if let Some(value) = boxed_inner_value.downcast_ref::<T>() {
				return value;
			}
		}
		DynamicOptional::fail_for_inner_access::<T>()
	}

	pub fn get_mut<T: 'static>(&mut self) -> &mut T {
		if let Some(boxed_inner_value) = &mut self.inner {
			if let Some(value) = boxed_inner_value.downcast_mut::<T>() {
				return value;
			}
		}
		DynamicOptional::fail_for_inner_access::<T>()
	}
}
