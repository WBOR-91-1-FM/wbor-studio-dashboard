pub type MaybeError = GenericResult<()>; // TODO: try to add line number info to the error
pub type GenericResult<T> = Result<T, Box<dyn std::error::Error>>;
