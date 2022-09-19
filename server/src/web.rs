use std::net::SocketAddr;

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

async fn log(req: Request<Body>, next: Next<Body>) -> Response {
	let addr = req.extensions().get::<ConnectInfo<SocketAddr>>();
	let str = addr.map_or("None".to_string(), |a| format!("{a:?}"));
	println!("Got from {str}\nRequest: {} {}", req.method(), req.uri());
	next.run(req).await
}

pub(super) fn make_service() -> Router<Body> {
	let redirect = get(|| async { Redirect::permanent("/index.xhtml") });
	web_frontend::get_router()
		.route("/", redirect)
		.nest("/api", api::make_router())
		.layer(from_fn(log))
}

mod api {
	use axum::routing::get;
	use axum::Router;
	use hyper::Body;

	pub(crate) fn make_router() -> Router<Body> {
		Router::new().route("/status", get(|| async { "Status" }))
	}
}

#[cfg(test)]
mod test {
	use std::error::Error;

	use hyper::service::Service;
	use hyper::{Body, Request, StatusCode};

	use tower::util::ServiceExt;

	#[tokio::test]
	async fn api_status_returns_200() -> Result<(), Box<dyn Error>> {
		let mut service = super::make_service();
		let request = Request::builder().uri("/api/status").body(Body::empty())?;
		let response = service.ready().await?.call(request).await?;

		assert_eq!(response.status(), StatusCode::OK);
		Ok(())
	}
}
