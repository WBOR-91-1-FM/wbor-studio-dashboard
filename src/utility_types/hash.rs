use std::hash::{Hash, Hasher};

pub fn hash_obj<T: Hash>(obj: &T) -> u64 {
	let mut hasher = std::hash::DefaultHasher::new();
	obj.hash(&mut hasher);
	hasher.finish()
}
