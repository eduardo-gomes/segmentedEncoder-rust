use std::net::SocketAddr;
use std::sync::Arc;

use axum::handler::Handler;
use axum::response::Redirect;
use axum::routing::get;
use axum::{
	body::Body,
	extract::ConnectInfo,
	middleware::{from_fn, Next},
	response::Response,
	Router,
};
use hyper::{Request, StatusCode};

use crate::State;

async fn log(req: Request<Body>, next: Next<Body>) -> Response {
	let addr = req.extensions().get::<ConnectInfo<SocketAddr>>();
	let str = addr.map_or("None".to_string(), |a| format!("{a:?}"));
	println!("Got from {str}\nRequest: {} {}", req.method(), req.uri());
	next.run(req).await
}

pub(super) fn make_service(state: Arc<State>) -> Router<Body> {
	let redirect = get(|| async { Redirect::permanent("/index.xhtml") });
	async fn fallback() -> (StatusCode, &'static str) {
		(StatusCode::NOT_FOUND, "Not found")
	}
	web_frontend::get_router()
		.route("/", redirect)
		.nest("/api", api::make_router(state))
		.layer(from_fn(log))
		.fallback(fallback.into_service())
}

mod api {
	use std::sync::Arc;

	use axum::extract::Path;
	use axum::http::{HeaderMap, Request};
	use axum::routing::{get, post};
	use axum::{Extension, Router};
	use hyper::{Body, Response, StatusCode};
	use uuid::Uuid;

	use crate::job_manager::{JobManagerLock, JobManagerUtils};
	use crate::jobs::{JobParams, Source};
	use crate::storage::stream::read_to_stream;
	use crate::State;

	fn parse_job(headers: &HeaderMap) -> Result<JobParams, String> {
		let get_opt = |header| {
			headers
				.get(header)
				.map(|val| {
					val.to_str()
						.map_err(|e| format!("{header} has invalid value: {e}"))
				})
				.transpose()
		};
		let encoder = get_opt("video_encoder")?;
		let video_args = get_opt("video_args")?;
		let audio_encoder = get_opt("audio_encoder")?;
		let audio_args = get_opt("audio_args")?;
		match encoder {
			None => Err("Missing video encoder".to_string()),
			Some(encoder) => Ok(JobParams {
				video_encoder: encoder.to_string(),
				video_args: video_args.map(String::from),
				audio_encoder: audio_encoder.map(String::from),
				audio_args: audio_args.map(String::from),
			}),
		}
	}

	async fn job_post(state: Extension<Arc<State>>, req: Request<Body>) -> Response<Body> {
		dbg!(req.headers());
		let headers = req.headers();
		let params = parse_job(headers).map_err(|str| {
			Response::builder()
				.status(StatusCode::BAD_REQUEST)
				.body(Body::from(str))
				.unwrap()
		});
		let handle_post = |params| async {
			let job = state.manager.create_job(req.into_body(), params).await;
			match job {
				Ok((uuid, _)) => Response::builder()
					.status(StatusCode::OK)
					.body(Body::from(uuid.as_hyphenated().to_string()))
					.unwrap(),
				Err(e) => Response::builder()
					.status(StatusCode::INTERNAL_SERVER_ERROR)
					.body(Body::from(format!("Failed to create job: {e}")))
					.unwrap(),
			}
		};
		match params {
			Err(res) => res,
			Ok(params) => handle_post(params).await,
		}
	}

	async fn job_source(Path(job_id): Path<Uuid>, state: Extension<Arc<State>>) -> Response<Body> {
		let job = {
			let lock = state.manager.read().await;
			lock.get_job(&job_id)
		};
		match job {
			Some(job) => {
				let source = job.read().await.source.clone();
				async fn send_local(state: &JobManagerLock, uuid: &Uuid) -> Response<Body> {
					let file = state.read().await.storage.get_file(uuid).await;
					match file {
						Ok(file) => Response::builder()
							.status(StatusCode::OK)
							//Uses Transfer-Encoding: chunked if Content-Length is not specified
							.body(Body::wrap_stream(read_to_stream(file)))
							.unwrap(),
						Err(e) => Response::builder()
							.status(StatusCode::INTERNAL_SERVER_ERROR)
							.body(Body::from(format!("Failed to read file: {e}")))
							.unwrap(),
					}
				}
				match source {
					Source::Local(uuid) => send_local(&state.manager, &uuid).await,
				}
			}
			None => Response::builder()
				.status(StatusCode::NOT_FOUND)
				.body(Body::from("Not found"))
				.unwrap(),
		}
	}

	async fn job_info(Path(job_id): Path<Uuid>, state: Extension<Arc<State>>) -> Response<Body> {
		match state.manager.read().await.get_job(&job_id) {
			None => Response::builder()
				.status(StatusCode::NOT_FOUND)
				.body(Body::from("Not found"))
				.unwrap(),
			Some(job) => {
				let params = &job.read().await.parameters;
				let string = format!("{params:#?}");
				println!("Info: {string}");
				Response::new(Body::from(string))
			}
		}
	}

	pub(crate) fn make_router(state: Arc<State>) -> Router<Body> {
		Router::new()
			.route("/status", get(get_status))
			.route("/jobs", post(job_post))
			.route("/jobs/:job_id/source", get(job_source))
			.route("/jobs/:job_id/info", get(job_info))
			.layer(Extension(state))
	}

	async fn get_status(state: Extension<Arc<State>>) -> Response<Body> {
		let manager = state.manager.read().await;
		let mut status = manager.status();
		let client_status = {
			match state.grpc.upgrade() {
				None => "GRPC_SERVICE WAS DROPPED".to_string(),
				Some(service) => service.read().await.status(),
			}
		};
		status.push('\n');
		status.push_str(&client_status);
		Response::new(Body::from(status))
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

	use crate::{State, Storage, WEBM_SAMPLE};

	fn make_service() -> Router<Body> {
		use std::sync::Arc;
		let state = {
			use crate::job_manager::JobManager;
			use tokio::sync::RwLock;
			let storage = Storage::new().unwrap();
			let manager_lock = RwLock::new(JobManager::new(storage));
			let service_lock = Arc::new(crate::client_interface::Service::new().into_lock());
			let weak_service = Arc::downgrade(&service_lock);

			//to use weak_service, we should keep the arc
			Arc::new(State {
				manager: manager_lock,
				grpc: weak_service,
			})
		};
		super::make_service(state)
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

	#[tokio::test]
	async fn api_job_info_contains_params() -> Result<(), Box<dyn Error>> {
		let mut service = make_service();
		let mut headers = HeaderMap::new();
		let video_encoder = "libx265";
		let video_args = "-crf 24";
		let audio_encoder = "libopus";
		let audio_args = "-b:a 96k";
		headers.insert("video_encoder", video_encoder.parse().unwrap());
		headers.insert("video_args", video_args.parse().unwrap());
		headers.insert("audio_encoder", audio_encoder.parse().unwrap());
		headers.insert("audio_args", audio_args.parse().unwrap());
		let id = post_job_ang_get_uuid(&mut service, &headers).await?;

		let request = Request::get(format!("/api/jobs/{id}/info"))
			.body(Body::empty())
			.unwrap();
		let response = service.oneshot(request).await?;
		assert_eq!(response.status(), StatusCode::OK);
		let content = hyper::body::to_bytes(response.into_body()).await?;
		let content = String::from_utf8(content.to_vec())?;
		assert!(content.contains(video_encoder));
		assert!(content.contains(video_args));
		assert!(content.contains(audio_encoder));
		assert!(content.contains(audio_args));
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

	async fn post_job_ang_get_uuid(
		service: &mut Router,
		headers: &HeaderMap,
	) -> Result<Uuid, Box<dyn Error>> {
		let request = build_job_request_with_headers(headers)?;
		let response = service.ready().await?.call(request).await?;
		let uuid = hyper::body::to_bytes(response.into_body()).await?;
		let uuid = String::from_utf8(uuid.to_vec()).map_err(|_| "Did not return UTF-8")?;
		let uuid = Uuid::parse_str(&uuid)?;
		Ok(uuid)
	}

	#[tokio::test]
	async fn post_job_response_is_uuid() -> Result<(), Box<dyn Error>> {
		let mut service = make_service();
		let mut headers = HeaderMap::new();
		headers.insert("video_encoder", "libx264".parse().unwrap());
		let uuid = post_job_ang_get_uuid(&mut service, &headers).await?;
		assert!(!uuid.is_nil());
		Ok(())
	}

	#[tokio::test]
	async fn after_posting_job_status_contains_job_id() -> Result<(), Box<dyn Error>> {
		let mut service = make_service();
		let mut headers = HeaderMap::new();
		headers.insert("video_encoder", "libx264".parse().unwrap());
		let job_id = post_job_ang_get_uuid(&mut service, &headers).await?;

		let status_request = Request::builder()
			.uri("/api/status")
			.body(Body::empty())
			.unwrap();
		let response = service.ready().await?.call(status_request).await?;
		let status = hyper::body::to_bytes(response.into_body()).await?;
		let status = String::from_utf8(status.to_vec())?;
		assert!(
			status.contains(&job_id.as_hyphenated().to_string()),
			"'{status}' should contain the job id '{job_id}'"
		);
		Ok(())
	}

	#[tokio::test]
	async fn get_job_source_200() -> Result<(), Box<dyn Error>> {
		let mut service = make_service();
		let mut headers = HeaderMap::new();
		headers.insert("video_encoder", "libx264".parse()?);
		let job_id = post_job_ang_get_uuid(&mut service, &headers).await?;

		let uri = format!("/api/jobs/{job_id}/source");
		let request = Request::get(uri).body(Body::empty()).unwrap();
		let response = service.ready().await?.call(request).await?;
		assert_eq!(response.status(), StatusCode::OK);
		Ok(())
	}

	#[tokio::test]
	async fn get_job_source_same_as_input() -> Result<(), Box<dyn Error>> {
		let mut service = make_service();
		let mut headers = HeaderMap::new();
		headers.insert("video_encoder", "libx264".parse()?);
		let job_id = post_job_ang_get_uuid(&mut service, &headers).await?;

		let uri = format!("/api/jobs/{job_id}/source");
		let request = Request::get(uri).body(Body::empty()).unwrap();
		let response = service.ready().await?.call(request).await?;
		assert_eq!(response.status(), StatusCode::OK);
		let content = hyper::body::to_bytes(response.into_body()).await?;
		assert_eq!(content, WEBM_SAMPLE.as_slice());
		Ok(())
	}

	#[tokio::test]
	async fn get_job_source_unknown_job_404() -> Result<(), Box<dyn Error>> {
		let service = make_service();
		let uuid = Uuid::new_v4();
		let uri = format!("/api/jobs/{uuid}/source");
		let request = Request::get(uri).body(Body::empty()).unwrap();
		let response = service.oneshot(request).await?;
		assert_eq!(response.status(), StatusCode::NOT_FOUND);
		Ok(())
	}

	#[tokio::test]
	async fn non_existent_page_returns_404_with_text() {
		let service = make_service();
		let request = Request::get("/non_existent_page")
			.body(Body::empty())
			.unwrap();
		let response = service.oneshot(request).await.unwrap();
		assert_eq!(response.status(), StatusCode::NOT_FOUND);
		let content = hyper::body::to_bytes(response.into_body()).await.unwrap();
		let string = String::from_utf8(content.to_vec()).unwrap();
		assert!(!string.is_empty());
	}
}
