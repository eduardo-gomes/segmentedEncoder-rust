//! Api based on api.yaml spec

use axum::http::{HeaderMap, HeaderName, StatusCode};
use axum::routing::get;
use axum::Router;

pub fn make_router() -> Router {
	Router::new()
		.route("/version", get(|| async { env!("CARGO_PKG_VERSION") }))
		.route("/login", get(login))
}

async fn login(header_map: HeaderMap) -> StatusCode {
	let credentials = header_map
		.get(HeaderName::from_static("credentials"))
		.map(|v| v.to_str())
		.transpose()
		.unwrap_or_default();
	match credentials {
		None => StatusCode::BAD_REQUEST,
		Some(_) => StatusCode::FORBIDDEN,
	}
}

#[cfg(test)]
mod test {
	use axum::http::{HeaderName, HeaderValue, StatusCode};
	use axum_test::TestServer;

	use crate::api::make_router;

	fn test_server() -> TestServer {
		TestServer::new(make_router()).unwrap()
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
}
