pub type MaybeError = GenericResult<()>;
pub type GenericResult<T> = Result<T, Box<dyn std::error::Error>>;
