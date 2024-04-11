//! Api based on api.yaml spec

use axum::routing::get;
use axum::Router;

pub fn make_router() -> Router {
	Router::new().route("/version", get(|| async { env!("CARGO_PKG_VERSION") }))
}

#[cfg(test)]
mod test {
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
}
