//! Exposes internal state for debugging this application
use std::fmt::{Debug, Formatter};
use std::sync::Arc;

use axum::http::Response;
use axum::Extension;
use hyper::Body;

use crate::client_interface::ServiceLock;
use crate::storage::Storage;
use crate::State;

impl Debug for Storage {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		let result = self.list_files_sync();
		f.debug_struct("Storage").field("files", &result).finish()
	}
}

impl Debug for ServiceLock {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		f.write_str("Unimplemented")
	}
}

impl Debug for State {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("State")
			.field("manager", &self.manager)
			.field("grpc", &self.grpc)
			.field("storage", &self.storage)
			.finish()
	}
}

pub(crate) async fn web_debug(state: Extension<Arc<State>>) -> Response<Body> {
	let str = format!("WEB_DEBUG:\n{:#?}", state.0);
	Response::builder().body(Body::from(str)).unwrap()
}
