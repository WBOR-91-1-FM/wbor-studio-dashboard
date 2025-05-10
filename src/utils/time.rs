use crate::utils::generic_result::*;

use chrono::DateTime;
pub use chrono::Timelike;

//////////

pub type Duration = chrono::Duration;

pub type LocalTimezone = chrono::Local;
pub type ReferenceTimezone = chrono::Utc;

type LocalTimestamp = DateTime<LocalTimezone>;
pub type ReferenceTimestamp = DateTime<ReferenceTimezone>;

//////////

pub fn get_local_time() -> LocalTimestamp {LocalTimezone::now()}
pub fn get_reference_time() -> ReferenceTimestamp {ReferenceTimezone::now()}

pub fn parse_rfc3339_timestamp(s: &str) -> GenericResult<ReferenceTimestamp> {
	DateTime::parse_from_rfc3339(s).map(|fixed_offset| fixed_offset.into()).to_generic_result()
}

////////// These are for when you have strings implementing `Deserialize` and you want to get direct `DateTime` objects

pub mod serde_parse {
	use chrono::{DateTime, ParseResult};
	use serde::{Deserializer, Deserialize};

	use super::ReferenceTimestamp;

	///////////

	fn timestamp<'de, D, S1, S2: From<S1>>
		(as_string: S1, parser: fn(S2) -> ParseResult<DateTime<chrono::FixedOffset>>)
		-> Result<ReferenceTimestamp, D::Error> where D: Deserializer<'de> {

		match parser(as_string.into()) {
			Ok(fixed_offset) => Ok(fixed_offset.into()),
			Err(err) => Err(serde::de::Error::custom(err))
		}
	}

	fn spinitron_timestamp_parser(mut string: String) -> ParseResult<DateTime<chrono::FixedOffset>> {
		string.insert(string.len() - 2, ':'); // Spinitron's date formatting needs to be modified a bit
		DateTime::parse_from_rfc3339(&string)
	}

	//////////

	pub fn rfc2822_timestamp<'de, D>(deserializer: D) -> Result<ReferenceTimestamp, D::Error> where D: Deserializer<'de> {
		timestamp::<D, _, _>(String::deserialize(deserializer)?.as_str(), DateTime::parse_from_rfc2822)
	}

	pub fn spinitron_timestamp<'de, D>(deserializer: D) -> Result<ReferenceTimestamp, D::Error> where D: Deserializer<'de> {
		timestamp::<D, _, _>(String::deserialize(deserializer)?, spinitron_timestamp_parser)
	}

	// This handles empty and null timestamps
	pub fn maybe_spinitron_timestamp<'de, D>(deserializer: D) -> Result<Option<ReferenceTimestamp>, D::Error> where D: Deserializer<'de> {
		match String::deserialize(deserializer) {
			Ok(as_string) => {
				if as_string.is_empty() {
					log::error!("Rare spin end-time situation #2: the field is empty.");
					Ok(None)
				}
				else {
					timestamp::<D, _, _>(as_string, spinitron_timestamp_parser).map(Some)
				}
			},

			// There is one other rare case than this one, which is when the field just isn't present at all
			Err(err) => {
				log::error!("Rare spin end-time situation #3: the field is null. Error: '{err}'.");
				Ok(None)
			}
		}
	}

}
