use crate::utility_types::generic_result::*;

use chrono::DateTime;
pub use chrono::Timelike;

//////////

pub type Duration = chrono::Duration;

pub type ReferenceTimezone = chrono::Utc;
pub type LocalTimezone = chrono::Local;

pub type ReferenceTimestamp = DateTime<ReferenceTimezone>;
type LocalTimestamp = DateTime<LocalTimezone>;
type AnyZoneTimestamp = DateTime<chrono::FixedOffset>;

//////////

pub fn parse_time_from_rfc2822(s: &str) -> GenericResult<AnyZoneTimestamp> {
	DateTime::parse_from_rfc2822(s).to_generic_result()
}

pub fn parse_time_from_rfc3339(s: &str) -> GenericResult<AnyZoneTimestamp> {
	DateTime::parse_from_rfc3339(s).to_generic_result()
}

pub fn get_reference_time() -> ReferenceTimestamp {
	ReferenceTimezone::now()
}

pub fn get_local_time() -> LocalTimestamp {
	LocalTimezone::now()
}
