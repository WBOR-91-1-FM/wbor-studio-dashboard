pub type GenericResult<T> = Result<T, Box<dyn std::error::Error>>;
pub type SendableGenericResult<T> = Result<T, String>;

pub fn make_sendable<T>(result: GenericResult<T>) -> SendableGenericResult<T> {
    match result {
        Ok(inner) => Ok(inner),
        Err(err) => Err(err.to_string())
    }
}
