use std::net::SocketAddr;

use multiplex_tonic_hyper::MakeMultiplexer;
use tower::make::Shared;

use status::status_reporter_server::StatusReporterServer;

pub mod status {
	tonic::include_proto!("status");
}

mod grpc;

pub mod web {
	use std::net::SocketAddr;

	use axum::{
		body::Body,
		extract::ConnectInfo,
		middleware::{from_fn, Next},
		response::Response,
		routing::get,
		Router,
	};
	use hyper::Request;

	use crate::grpc::StatusKeeper;

	async fn log(req: Request<Body>, next: Next<Body>) -> Response {
		let addr = req.extensions().get::<ConnectInfo<SocketAddr>>();
		println!(
			"Got from{addr:?}\nRequest: {} {} {:?}",
			req.method(),
			req.uri(),
			req.version()
		);
		next.run(req).await
	}

	pub(super) fn make_service(status_keeper: &StatusKeeper) -> Router<Body> {
		let fun = |keeper: StatusKeeper| format!("{:#?}", keeper.get_latest_report());
		web_packer::include_web_static!()
			.route("/", get(|| async { "Hello, world!" }))
			.route(
				"/latest",
				get({
					let keeper = status_keeper.clone();
					move || {
						let keeper = keeper.clone();
						async move { fun(keeper.clone()) }
					}
				}),
			)
			.layer(from_fn(log))
	}
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	let addr = "[::]:8888".parse().unwrap();

	let status_reporter_service = grpc::StatusKeeper::new();

	let web_service = web::make_service(&status_reporter_service)
		.into_make_service_with_connect_info::<SocketAddr>();

	let svc = Shared::new(StatusReporterServer::new(status_reporter_service));

	let multi = MakeMultiplexer::new(svc, web_service);

	println!("Starting server on http://{:?}", addr);

	let server = hyper::Server::bind(&addr).serve(multi);

	server.await?;

	Ok(())
}
