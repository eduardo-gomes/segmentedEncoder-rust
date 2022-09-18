use std::net::SocketAddr;

pub mod web {
	use std::net::SocketAddr;

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
		web_frontend::get_router().layer(from_fn(log))
	}
}

async fn shutdown_signal() {
	// Wait for the CTRL+C signal
	tokio::signal::ctrl_c()
		.await
		.expect("failed to install CTRL+C signal handler");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	let addr = "[::]:8888".parse().unwrap();

	let web_service = web::make_service().into_make_service_with_connect_info::<SocketAddr>();

	println!("Starting server on http://{:?}", addr);
	let server = hyper::Server::bind(&addr).serve(web_service);
	let graceful = server.with_graceful_shutdown(shutdown_signal());

	graceful.await?;
	Ok(())
}
