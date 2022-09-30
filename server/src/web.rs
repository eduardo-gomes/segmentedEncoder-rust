use std::net::SocketAddr;
use std::sync::{Arc, RwLock};

use axum::response::Redirect;
use axum::routing::get;
use axum::{
	body::Body,
	extract::ConnectInfo,
	middleware::{from_fn, Next},
	response::Response,
	Router,
};
use hyper::Request;

use crate::job_manager::JobManager;

async fn log(req: Request<Body>, next: Next<Body>) -> Response {
	let addr = req.extensions().get::<ConnectInfo<SocketAddr>>();
	let str = addr.map_or("None".to_string(), |a| format!("{a:?}"));
	println!("Got from {str}\nRequest: {} {}", req.method(), req.uri());
	next.run(req).await
}

pub(super) fn make_service(manager: Arc<RwLock<JobManager>>) -> Router<Body> {
	let redirect = get(|| async { Redirect::permanent("/index.xhtml") });
	web_frontend::get_router()
		.route("/", redirect)
		.nest("/api", api::make_router(manager))
		.layer(from_fn(log))
}

mod api {
	use std::sync::{Arc, RwLock};

	use axum::http::{HeaderMap, Request};
	use axum::routing::{get, post};
	use axum::{Extension, Router};
	use hyper::{Body, Response, StatusCode};

	use crate::job_manager::JobManager;
	use crate::jobs::{Job, JobParams, Source};

	fn parse_job(headers: &HeaderMap) -> Result<JobParams, &'static str> {
		let encoder = headers
			.get("video_encoder")
			.map(|val| val.to_str().map_err(|_| "video_codec has invalid value"))
			.transpose()?;
		match encoder {
			None => return Err("Missing video encoder"),
			Some(encoder) => Ok(JobParams {
				video_encoder: encoder.to_string(),
			}),
		}
	}

	async fn job_post(
		state: Extension<Arc<RwLock<JobManager>>>,
		req: Request<Body>,
	) -> Response<Body> {
		dbg!(req.headers());
		let headers = req.headers();
		let params = parse_job(headers);
		match params {
			Err(str) => Response::builder()
				.status(StatusCode::BAD_REQUEST)
				.body(Body::from(str))
				.unwrap(),
			Ok(params) => {
				let job = Job::new(Source::Local(), params);
				match state.0.write() {
					Ok(mut manager) => {
						let (uuid, _) = manager.add_job(job);
						Response::builder()
							.status(StatusCode::OK)
							.body(Body::from(uuid.as_hyphenated().to_string()))
							.unwrap()
					}
					Err(e) => Response::builder()
						.status(StatusCode::INTERNAL_SERVER_ERROR)
						.body(Body::from(format!("Job manager became poisoned!\n{e}")))
						.unwrap(),
				}
			}
		}
	}

	pub(crate) fn make_router(job_manager: Arc<RwLock<JobManager>>) -> Router<Body> {
		Router::new()
			.route("/status", get(get_status))
			.route("/jobs", post(job_post))
			.layer(Extension(job_manager))
	}

	async fn get_status(state: Extension<Arc<RwLock<JobManager>>>) -> Response<Body> {
		match state.read() {
			Ok(job_manager) => {
				let status = job_manager.status();
				Response::new(Body::from(status))
			}
			Err(e) => Response::builder()
				.status(StatusCode::INTERNAL_SERVER_ERROR)
				.body(Body::from(format!("Job manager became poisoned!\n{e}")))
				.unwrap(),
		}
	}
}

#[cfg(test)]
mod test {
	use std::error::Error;

	use axum::Router;
	use hyper::header::CONTENT_TYPE;
	use hyper::service::Service;
	use hyper::{http, Body, HeaderMap, Method, Request, StatusCode};
	use tower::util::ServiceExt;
	use uuid::Uuid;

	fn make_service() -> Router<Body> {
		use crate::job_manager::JobManager;
		use std::sync::{Arc, RwLock};
		let manager = Arc::new(RwLock::new(JobManager::new()));
		super::make_service(manager)
	}

	#[tokio::test]
	async fn api_status_returns_200() -> Result<(), Box<dyn Error>> {
		let mut service = make_service();
		let request = Request::builder().uri("/api/status").body(Body::empty())?;
		let response = service.ready().await?.call(request).await?;

		assert_eq!(response.status(), StatusCode::OK);
		Ok(())
	}

	//Sample webm file, to use on tests
	const WEBM_SAMPLE: [u8; 185] = [
		0x1a, 0x45, 0xdf, 0xa3, 0x40, 0x20, 0x42, 0x86, 0x81, 0x01, 0x42, 0xf7, 0x81, 0x01, 0x42,
		0xf2, 0x81, 0x04, 0x42, 0xf3, 0x81, 0x08, 0x42, 0x82, 0x40, 0x04, 0x77, 0x65, 0x62, 0x6d,
		0x42, 0x87, 0x81, 0x02, 0x42, 0x85, 0x81, 0x02, 0x18, 0x53, 0x80, 0x67, 0x40, 0x8d, 0x15,
		0x49, 0xa9, 0x66, 0x40, 0x28, 0x2a, 0xd7, 0xb1, 0x40, 0x03, 0x0f, 0x42, 0x40, 0x4d, 0x80,
		0x40, 0x06, 0x77, 0x68, 0x61, 0x6d, 0x6d, 0x79, 0x57, 0x41, 0x40, 0x06, 0x77, 0x68, 0x61,
		0x6d, 0x6d, 0x79, 0x44, 0x89, 0x40, 0x08, 0x40, 0x8f, 0x40, 0x00, 0x00, 0x00, 0x00, 0x00,
		0x16, 0x54, 0xae, 0x6b, 0x40, 0x31, 0xae, 0x40, 0x2e, 0xd7, 0x81, 0x01, 0x63, 0xc5, 0x81,
		0x01, 0x9c, 0x81, 0x00, 0x22, 0xb5, 0x9c, 0x40, 0x03, 0x75, 0x6e, 0x64, 0x86, 0x40, 0x05,
		0x56, 0x5f, 0x56, 0x50, 0x38, 0x25, 0x86, 0x88, 0x40, 0x03, 0x56, 0x50, 0x38, 0x83, 0x81,
		0x01, 0xe0, 0x40, 0x06, 0xb0, 0x81, 0x08, 0xba, 0x81, 0x08, 0x1f, 0x43, 0xb6, 0x75, 0x40,
		0x22, 0xe7, 0x81, 0x00, 0xa3, 0x40, 0x1c, 0x81, 0x00, 0x00, 0x80, 0x30, 0x01, 0x00, 0x9d,
		0x01, 0x2a, 0x08, 0x00, 0x08, 0x00, 0x01, 0x40, 0x26, 0x25, 0xa4, 0x00, 0x03, 0x70, 0x00,
		0xfe, 0xfc, 0xf4, 0x00, 0x00,
	];

	#[tokio::test]
	async fn api_post_job_empty() -> Result<(), Box<dyn Error>> {
		let mut service = make_service();
		let request = Request::builder()
			.uri("/api/jobs")
			.method(Method::POST)
			.body(Body::empty())?;
		let response = service.ready().await?.call(request).await?;

		assert_eq!(response.status(), StatusCode::BAD_REQUEST);
		Ok(())
	}

	#[tokio::test]
	async fn api_post_job_video_encoder_only() -> Result<(), Box<dyn Error>> {
		let mut service = make_service();
		let mut headers = HeaderMap::new();
		headers.insert("video_encoder", "libx264".parse().unwrap());
		let request = build_job_request_with_headers(&headers)?;
		let response = service.ready().await?.call(request).await?;

		assert_eq!(response.status(), StatusCode::OK);
		Ok(())
	}

	fn build_job_request_with_headers(headers: &HeaderMap) -> Result<Request<Body>, http::Error> {
		let mut request = Request::builder().uri("/api/jobs").method(Method::POST);
		for (name, value) in headers {
			request = request.header(name, value);
		}
		request
			.header(CONTENT_TYPE, "video/webm")
			.body(Body::from(WEBM_SAMPLE.as_slice()))
	}

	#[tokio::test]
	async fn api_post_job_video_encoder_invalid() -> Result<(), Box<dyn Error>> {
		let mut service = make_service();
		let mut headers = HeaderMap::new();
		headers.insert("video_encoder", "non ascii character: ç".parse().unwrap());
		let request = build_job_request_with_headers(&headers)?;
		let response = service.ready().await?.call(request).await?;

		assert_eq!(response.status(), StatusCode::BAD_REQUEST);
		Ok(())
	}

	#[tokio::test]
	async fn api_post_job_video_no_parameters() -> Result<(), Box<dyn Error>> {
		let mut service = make_service();
		let request = build_job_request_with_headers(&Default::default())?;
		let response = service.ready().await?.call(request).await?;

		assert_eq!(response.status(), StatusCode::BAD_REQUEST);
		Ok(())
	}

	#[tokio::test]
	async fn post_job_response_is_uuid() -> Result<(), Box<dyn Error>> {
		let mut service = make_service();
		let mut headers = HeaderMap::new();
		headers.insert("video_encoder", "libx264".parse().unwrap());
		let request = build_job_request_with_headers(&headers)?;
		let response = service.ready().await?.call(request).await?;
		let uuid = hyper::body::to_bytes(response.into_body()).await?;
		let uuid = String::from_utf8(uuid.to_vec()).expect("Did not return UTF-8");
		let uuid = Uuid::parse_str(&uuid)?;
		assert!(!uuid.is_nil());
		Ok(())
	}

	#[tokio::test]
	async fn after_posting_job_status_contains_job_id() -> Result<(), Box<dyn Error>> {
		let mut service = make_service();
		let mut headers = HeaderMap::new();
		headers.insert("video_encoder", "libx264".parse().unwrap());
		let request = build_job_request_with_headers(&headers)?;
		let response = service.ready().await?.call(request).await?;
		let job_id = hyper::body::to_bytes(response.into_body()).await?;
		let job_id = String::from_utf8(job_id.to_vec())?;

		let status_request = Request::builder()
			.uri("/api/status")
			.body(Body::empty())
			.unwrap();
		let response = service.ready().await?.call(status_request).await?;
		let status = hyper::body::to_bytes(response.into_body()).await?;
		let status = String::from_utf8(status.to_vec())?;
		assert!(
			status.contains(&job_id),
			"'{status}' should contain the job id '{job_id}'"
		);
		Ok(())
	}
}
