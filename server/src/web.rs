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
	use uuid::Uuid;

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
				let job = Job::new(Source::Local(Uuid::nil()), params);
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

	use crate::{Storage, WEBM_SAMPLE};

	fn make_service() -> Router<Body> {
		use crate::job_manager::JobManager;
		use std::sync::{Arc, RwLock};
		let storage = Storage::new().unwrap();
		let manager = Arc::new(RwLock::new(JobManager::new(storage)));
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
