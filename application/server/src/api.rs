//! Api based on api.yaml spec

use std::sync::Arc;

use axum::extract::State;
use axum::http::{HeaderMap, HeaderName, StatusCode};
use axum::routing::{get, post};
use axum::Router;

use auth_module::AuthenticationHandler;

#[derive(Clone, Default)]
pub struct AppState {
	credential: String,
	auth_handler: Arc<auth_module::LocalAuthenticator>,
}

impl AppState {
	pub fn with_cred(cred: &str) -> AppState {
		AppState {
			credential: cred.into(),
			..Default::default()
		}
	}
}

pub fn make_router(state: AppState) -> Router {
	Router::new()
		.route("/version", get(|| async { env!("CARGO_PKG_VERSION") }))
		.route("/login", get(login))
		.route("/job", post(job_post))
		.with_state(state)
}

async fn login(
	State(state): State<AppState>,
	header_map: HeaderMap,
) -> Result<(StatusCode, String), StatusCode> {
	let credentials = header_map
		.get(HeaderName::from_static("credentials"))
		.map(|v| v.to_str())
		.transpose()
		.unwrap_or_default();
	match credentials {
		None => Err(StatusCode::BAD_REQUEST),
		Some(provided) => match provided == state.credential {
			true => Ok((StatusCode::OK, state.auth_handler.new_token().await)),
			false => Err(StatusCode::FORBIDDEN),
		},
	}
}

async fn job_post() -> StatusCode {
	StatusCode::FORBIDDEN
}

#[cfg(test)]
mod test {
	use axum::http::{HeaderName, HeaderValue, StatusCode};
	use axum_test::TestServer;

	use auth_module::AuthenticationHandler;

	use crate::api::{make_router, AppState};

	const TEST_CRED: &str = "test_auth";
	fn test_server() -> TestServer {
		test_server_state().0
	}
	fn test_server_state() -> (TestServer, AppState) {
		let state = AppState::with_cred(TEST_CRED);
		(TestServer::new(make_router(state.clone())).unwrap(), state)
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
	async fn login_will_return_a_token_recognizable_by_auth_handler() {
		let (server, state) = test_server_state();
		let token = server
			.get("/login")
			.add_header(
				HeaderName::from_static("credentials"),
				HeaderValue::from_static(TEST_CRED),
			)
			.await
			.text();
		let valid = state
			.auth_handler
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
}
