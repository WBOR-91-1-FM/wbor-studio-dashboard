pub type DynamicOptional = Option<Box<dyn std::any::Any>>;

pub fn get_inner_value<'a, T: 'static>(value: &'a mut DynamicOptional) -> &'a mut T {
	if let Some(boxed_inner_value) = value {
		if let Some(value) = boxed_inner_value.downcast_mut::<T>() {
			return value;
		}
	}

	panic!(
		"Could not access the inner value of a DynamicOptional, given the expected type '{}'",
		std::any::type_name::<T>()
	);
}
