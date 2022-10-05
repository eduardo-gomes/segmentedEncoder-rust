use std::time::Duration;

use crate::status::Duration as StatusDuration;

pub mod status {
	tonic::include_proto!("status");
}

impl From<Duration> for StatusDuration {
	fn from(duration: Duration) -> Self {
		StatusDuration {
			seconds: duration.as_secs(),
			nanoseconds: duration.subsec_nanos(),
		}
	}
}
