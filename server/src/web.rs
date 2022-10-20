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

mod api;
