use axum::Router;

pub mod web;

/// Temporary function to 'build' the service.
/// Will be replaced with a proper builder to set service proprieties.
pub fn make_service() -> Router {
	web::make_service()
}

mod storage;

mod jobs {
	pub struct JobParams {
		video_encoder: String,
	}

	impl JobParams {
		#[cfg(test)]
		pub(crate) fn sample_params() -> Self {
			JobParams {
				video_encoder: "libsvtav1".to_string(),
			}
		}
	}
	#[derive(Debug)]
	pub struct Job();
}

#[allow(dead_code)]
mod job_manager;
