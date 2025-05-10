pub type MaybeError = GenericResult<()>;
pub type GenericResult<T> = anyhow::Result<T>;

macro_rules! error_msg {
	($fmt:expr $(, $($arg:tt)*)?) => {
		Err(anyhow::anyhow!($fmt $(, $($arg)*)?))
	};
}

pub(crate) use error_msg;

pub trait ToGenericResult<T, E> {
	fn to_generic_result(self) -> GenericResult<T>;
}

impl<T, E> ToGenericResult<T, E> for Result<T, E>
	where E: std::fmt::Debug + std::fmt::Display + Send + Sync + 'static {

	fn to_generic_result(self) -> GenericResult<T> {
		self.map_err(anyhow::Error::msg)
	}
}
