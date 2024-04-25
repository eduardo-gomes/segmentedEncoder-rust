//! Api based on api.yaml spec

use std::sync::Arc;

use axum::body::Body;
use axum::extract::{FromRequestParts, State};
use axum::http::request::Parts;
use axum::http::{header, HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};

use auth_module::AuthenticationHandler;
use task::manager::Manager;
use task::{Input, JobOptions, JobSource, Options, Recipe, TaskSource};

use crate::storage::{MemStorage, Storage};

mod client;
mod utils;
mod worker;

pub trait AppState: Sync + Send {
	fn manager(&self) -> &impl Manager;
	fn auth_handler(&self) -> &impl AuthenticationHandler;
	fn storage(&self) -> &impl Storage;
	fn check_credential(&self, cred: &str) -> bool;
}

#[derive(Default)]
pub struct AppStateLocal {
	credential: String,
	_auth_handler: auth_module::LocalAuthenticator,
	_manager: task::manager::LocalJobManager,
	_storage: MemStorage,
}

impl AppState for AppStateLocal {
	fn manager(&self) -> &impl Manager {
		&self._manager
	}
	fn auth_handler(&self) -> &impl AuthenticationHandler {
		&self._auth_handler
	}
	fn storage(&self) -> &impl Storage {
		&self._storage
	}
	fn check_credential(&self, cred: &str) -> bool {
		self.credential == cred
	}
}

impl AppStateLocal {
	pub fn with_cred(cred: &str) -> AppStateLocal {
		AppStateLocal {
			credential: cred.into(),
			..Default::default()
		}
	}
}

struct AuthToken(String);

#[async_trait::async_trait]
impl<S: AppState> FromRequestParts<Arc<S>> for AuthToken {
	type Rejection = (StatusCode, &'static str);

	async fn from_request_parts(
		parts: &mut Parts,
		state: &Arc<S>,
	) -> Result<Self, Self::Rejection> {
		let header = parts
			.headers
			.get(header::AUTHORIZATION)
			.map(|v| v.to_str())
			.transpose()
			.unwrap_or_default()
			.ok_or((StatusCode::FORBIDDEN, "Missing authorization"))?
			.to_string();
		let auth = state
			.auth_handler()
			.is_valid(&header)
			.await
			.unwrap_or_default();
		auth.then_some(AuthToken(header))
			.ok_or((StatusCode::FORBIDDEN, "Bad authorization"))
	}
}

pub fn make_router<S: AppState + 'static>(state: Arc<S>) -> Router {
	Router::<Arc<S>>::new()
		.route(
			"/version",
			get(|| async { concat!("\"", env!("CARGO_PKG_VERSION"), "\"") }),
		)
		.route("/login", get(login))
		.route("/job", post(job_post))
		.route(
			"/job/:job_id/task/:task_id/input/0",
			get(worker::get_task_input),
		)
		.route(
			"/job/:job_id/task/:task_id/output",
			get(client::task_output_get).put(worker::put_task_output),
		)
		.route(
			"/job/:job_id/task/:task_id/status",
			post(worker::task_status_post),
		)
		.route("/job/:job_id/task", post(worker::task_post))
		.route("/job/:job_id/output", get(client::job_output_get))
		.route("/allocate_task", get(worker::allocate_task))
		.with_state(state)
}

async fn login<S: AppState>(
	State(state): State<Arc<S>>,
	header_map: HeaderMap,
) -> Result<Json<String>, StatusCode> {
	let credentials = header_map
		.get(HeaderName::from_static("credentials"))
		.map(|v| v.to_str())
		.transpose()
		.unwrap_or_default();
	match credentials {
		None => Err(StatusCode::BAD_REQUEST),
		Some(provided) => match state.check_credential(provided) {
			true => Ok(Json(state.auth_handler().new_token().await)),
			false => Err(StatusCode::FORBIDDEN),
		},
	}
}

async fn job_post<S: AppState>(
	State(state): State<Arc<S>>,
	_auth: AuthToken,
	headers: HeaderMap,
	body: Body,
) -> Result<impl IntoResponse, StatusCode> {
	let video_codec = headers
		.get(HeaderName::from_static("video_codec"))
		.map(HeaderValue::to_str)
		.transpose()
		.unwrap_or_default()
		.ok_or(StatusCode::BAD_REQUEST)?;
	let video_param: Vec<String> = headers
		.get_all(HeaderName::from_static("video_param"))
		.iter()
		.map(HeaderValue::to_str)
		.map(|v| v.map(String::from))
		.collect::<Result<Vec<_>, _>>()
		.or(Err(StatusCode::BAD_REQUEST))?;
	let input_id = state
		.storage()
		.body_to_new_file(body)
		.await
		.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
	let job_id = state
		.manager()
		.create_job(JobSource {
			input_id,
			options: JobOptions {
				video: Options {
					codec: Some(video_codec.to_string()),
					params: video_param,
				},
				audio: None,
			},
		})
		.await
		.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
	state
		.manager()
		.add_task_to_job(
			&job_id,
			TaskSource {
				inputs: vec![Input::source()],
				recipe: Recipe::Analysis(None),
			},
		)
		.await
		.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
	Ok((StatusCode::CREATED, job_id.to_string()))
}

#[cfg(test)]
mod test {
	use std::sync::Arc;

	use axum::body::Bytes;
	use axum::http::header::AUTHORIZATION;
	use axum::http::{HeaderName, HeaderValue, StatusCode};
	use axum_test::{TestRequest, TestServer};
	use tokio::io::AsyncReadExt;
	use uuid::Uuid;

	use auth_module::AuthenticationHandler;
	use task::manager::Manager;
	use task::Recipe;

	use crate::api::{make_router, AppState, AppStateLocal};
	use crate::storage::Storage;
	use crate::MKV_SAMPLE;

	pub(crate) const TEST_CRED: &str = "test_auth";

	pub(crate) fn test_server() -> TestServer {
		test_server_state().0
	}

	pub(crate) fn test_server_state() -> (TestServer, Arc<AppStateLocal>) {
		let state = Arc::new(AppStateLocal::with_cred(TEST_CRED));
		(
			TestServer::new(make_router::<AppStateLocal>(state.clone())).unwrap(),
			state,
		)
	}

	pub(crate) async fn test_server_auth() -> (TestServer, HeaderValue) {
		let (server, _, token) = test_server_state_auth().await;
		(server, token)
	}

	pub(crate) async fn test_server_state_auth() -> (TestServer, Arc<AppStateLocal>, HeaderValue) {
		test_server_state_auth_generic(Arc::new(AppStateLocal::with_cred(TEST_CRED))).await
	}

	pub(crate) async fn test_server_state_auth_generic<S: AppState + 'static>(
		state: Arc<S>,
	) -> (TestServer, Arc<S>, HeaderValue) {
		let (server, state) = (
			TestServer::new(make_router::<S>(state.clone())).unwrap(),
			state,
		);
		let token = state.auth_handler().new_token().await;
		let token: HeaderValue = token.parse().unwrap();
		(server, state, token)
	}

	#[tokio::test]
	async fn get_version_ok() {
		let server = test_server();
		let status = server.get("/version").await.status_code();
		assert!(status.is_success());
	}

	#[tokio::test]
	async fn get_version_contains_crate_version() {
		let server = test_server();
		let version = server.get("/version").await.text();
		let expected = env!("CARGO_PKG_VERSION");
		assert!(
			version.contains(expected),
			"Got {version}, expected {expected}"
		);
	}

	#[tokio::test]
	async fn get_version_is_json_string() {
		let server = test_server();
		let version: String = server.get("/version").await.json();
		assert!(!version.is_empty())
	}

	#[tokio::test]
	async fn get_login_without_auth_bad_request() {
		let server = test_server();
		let status = server.get("/login").await.status_code();
		assert_eq!(status, StatusCode::BAD_REQUEST);
	}

	#[tokio::test]
	async fn get_login_with_bad_auth_forbidden() {
		let server = test_server();
		let status = server
			.get("/login")
			.add_header(
				HeaderName::from_static("credentials"),
				HeaderValue::from_static("bad auth"),
			)
			.await
			.status_code();
		assert_eq!(status, StatusCode::FORBIDDEN);
	}

	#[tokio::test]
	async fn get_login_with_good_auth() {
		let server = test_server();
		let status = server
			.get("/login")
			.add_header(
				HeaderName::from_static("credentials"),
				HeaderValue::from_static(TEST_CRED),
			)
			.await
			.status_code();
		assert!(status.is_success());
	}

	#[tokio::test]
	async fn get_login_returns_text() {
		let server = test_server();
		let token = server
			.get("/login")
			.add_header(
				HeaderName::from_static("credentials"),
				HeaderValue::from_static(TEST_CRED),
			)
			.await
			.text();
		assert!(!token.is_empty());
	}

	#[tokio::test]
	async fn get_login_returns_json_string() {
		let server = test_server();
		let res = server
			.get("/login")
			.add_header(
				HeaderName::from_static("credentials"),
				HeaderValue::from_static(TEST_CRED),
			)
			.await;
		assert!(!res.json::<String>().is_empty())
	}

	#[tokio::test]
	async fn login_will_return_a_token_recognizable_by_auth_handler() {
		let (server, state) = test_server_state();
		let token = server
			.get("/login")
			.add_header(
				HeaderName::from_static("credentials"),
				HeaderValue::from_static(TEST_CRED),
			)
			.await
			.json::<String>();
		let valid = state
			.auth_handler()
			.is_valid(&token)
			.await
			.unwrap_or_default();
		assert!(valid);
	}

	#[tokio::test]
	async fn job_post_without_auth_forbidden() {
		let server = test_server();
		let status = server.post("/job").await.status_code();
		assert_eq!(status, StatusCode::FORBIDDEN)
	}

	#[tokio::test]
	async fn job_empty_post_with_auth_bad_request() {
		let server = test_server();
		let token: HeaderValue = server
			.get("/login")
			.add_header(
				HeaderName::from_static("credentials"),
				HeaderValue::from_static(TEST_CRED),
			)
			.await
			.json::<String>()
			.parse()
			.unwrap();
		let status = server
			.post("/job")
			.add_header(AUTHORIZATION, token)
			.await
			.status_code();
		assert_eq!(status, StatusCode::BAD_REQUEST)
	}

	#[tokio::test]
	async fn job_post_with_body_and_video_codec_created() {
		let (server, token) = test_server_auth().await;
		let status = server
			.post("/job")
			.add_header(AUTHORIZATION, token)
			.add_header(
				HeaderName::from_static("video_codec"),
				HeaderValue::from_static("libx264"),
			)
			.bytes(MKV_SAMPLE.as_slice().into())
			.await
			.status_code();
		assert_eq!(status, StatusCode::CREATED)
	}

	#[tokio::test]
	async fn job_post_returns_uuid() {
		let (server, token) = test_server_auth().await;
		let job_id = server
			.post("/job")
			.add_header(AUTHORIZATION, token)
			.add_header(
				HeaderName::from_static("video_codec"),
				HeaderValue::from_static("libx264"),
			)
			.bytes(MKV_SAMPLE.as_slice().into())
			.await
			.text();
		assert!(Uuid::parse_str(&job_id).is_ok())
	}

	fn make_post_job_request(
		server: TestServer,
		token: HeaderValue,
		options: task::Options,
		body: Bytes,
	) -> TestRequest {
		let mut req = server
			.post("/job")
			.add_header(AUTHORIZATION, token)
			.add_header(
				HeaderName::from_static("video_codec"),
				HeaderValue::from_str(
					options
						.codec
						.as_ref()
						.map(String::as_str)
						.unwrap_or("libx264"),
				)
				.unwrap(),
			)
			.bytes(body);
		let params = options
			.params
			.iter()
			.map(String::as_str)
			.map(HeaderValue::from_str)
			.map(|x| x.unwrap());
		for param in params {
			req = req.add_header(HeaderName::from_static("video_param"), param);
		}
		req
	}

	#[tokio::test]
	async fn job_post_creates_job_on_task_manager() {
		let (server, state, token) = test_server_state_auth().await;
		let job_options = task::Options {
			codec: Some("libx264".to_string()),
			params: vec![],
		};
		let job_id: Uuid =
			make_post_job_request(server, token, job_options, MKV_SAMPLE.as_slice().into())
				.await
				.text()
				.parse()
				.unwrap();
		let job = state.manager().get_job(&job_id).await.unwrap();
		assert!(job.is_some())
	}

	#[tokio::test]
	async fn job_post_creates_job_with_same_codec() {
		let (server, state, token) = test_server_state_auth().await;
		let job_options = task::Options {
			codec: Some("libx264".to_string()),
			params: vec![],
		};
		let job_id: Uuid = make_post_job_request(
			server,
			token,
			job_options.clone(),
			MKV_SAMPLE.as_slice().into(),
		)
		.await
		.text()
		.parse()
		.unwrap();
		let job = state
			.manager()
			.get_job(&job_id)
			.await
			.unwrap()
			.unwrap()
			.options;
		assert_eq!(job.video.codec, job_options.codec)
	}

	#[tokio::test]
	async fn job_post_creates_job_with_same_first_params() {
		let (server, state, token) = test_server_state_auth().await;
		let job_options = task::Options {
			codec: Some("libx264".to_string()),
			params: vec!["opt".to_string()],
		};
		let job_id: Uuid = make_post_job_request(
			server,
			token,
			job_options.clone(),
			MKV_SAMPLE.as_slice().into(),
		)
		.await
		.text()
		.parse()
		.unwrap();
		let job = state
			.manager()
			.get_job(&job_id)
			.await
			.unwrap()
			.unwrap()
			.options;
		assert_eq!(job.video.params[0], job_options.params[0])
	}

	#[tokio::test]
	async fn job_post_creates_job_with_multiple_params() {
		let (server, state, token) = test_server_state_auth().await;
		let job_options = task::Options {
			codec: Some("libx264".to_string()),
			params: vec!["opt1", "opt2", "opt3", "opt4"]
				.into_iter()
				.map(String::from)
				.collect(),
		};
		let job_id: Uuid = make_post_job_request(
			server,
			token,
			job_options.clone(),
			MKV_SAMPLE.as_slice().into(),
		)
		.await
		.text()
		.parse()
		.unwrap();
		let job = state
			.manager()
			.get_job(&job_id)
			.await
			.unwrap()
			.unwrap()
			.options;
		assert_eq!(job.video.params, job_options.params)
	}

	#[tokio::test]
	async fn job_post_body_will_be_saved_on_storage() {
		let (server, state, token) = test_server_state_auth().await;
		let job_options = task::Options {
			codec: Some("libx264".to_string()),
			params: vec![],
		};
		let job_id: Uuid = make_post_job_request(
			server,
			token,
			job_options.clone(),
			MKV_SAMPLE.as_slice().into(),
		)
		.await
		.text()
		.parse()
		.unwrap();
		let input = state
			.manager()
			.get_job(&job_id)
			.await
			.unwrap()
			.unwrap()
			.input_id;
		let mut read = state.storage().read_file(input).await.unwrap();
		let mut readed = Vec::new();
		AsyncReadExt::read_to_end(&mut read, &mut readed)
			.await
			.unwrap();
		assert_eq!(readed, MKV_SAMPLE);
	}

	#[tokio::test]
	async fn job_post_will_schedule_a_task() {
		let (server, state, token) = test_server_state_auth().await;
		let job_options = task::Options {
			codec: Some("libx264".to_string()),
			params: vec![],
		};
		let job_id: Uuid = make_post_job_request(
			server,
			token,
			job_options.clone(),
			MKV_SAMPLE.as_slice().into(),
		)
		.await
		.text()
		.parse()
		.unwrap();
		let task = state.manager().allocate_task().await.unwrap();
		assert!(task.is_some());
		let task = task.unwrap().recipe;
		assert!(matches!(task, Recipe::Analysis(_)))
	}
}
