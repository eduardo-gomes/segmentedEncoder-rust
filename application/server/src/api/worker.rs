//! Worker api
//!
//! Define the routes used by the workers to execute tasks

use axum::http::StatusCode;

pub(super) async fn allocate_task() -> StatusCode{
	StatusCode::NOT_FOUND
}

#[cfg(test)]
mod test {
	use axum::http::StatusCode;
	use super::super::test::*;

	#[tokio::test]
	async fn allocate_task_requires_auth() {
		let (server, _, _) = test_server_state_auth().await;
		let res = server.get("/allocate_task").await.status_code();
		assert_eq!(res, StatusCode::FORBIDDEN)
	}
}
