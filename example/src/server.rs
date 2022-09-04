use status::status_reporter_server::StatusReporterServer;
use tower::make::Shared;

pub mod status {
	tonic::include_proto!("status");
}

mod grpc;

pub mod web {
	use axum::{body::Body, routing::get, Router};

	use crate::grpc::StatusKeeper;

	pub(super) fn make_service(status_keeper: &StatusKeeper) -> axum::Router<Body> {
		let fun = |keeper: StatusKeeper| format!("{:#?}", keeper.get_latest_report());
		Router::new()
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
	}
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	let addr = "[::]:8888".parse().unwrap();

	let status_reporter_service = grpc::StatusKeeper::new();

	// let web_service = web::make_service(&status_reporter_service).into_make_service();

	let svc = Shared::new(StatusReporterServer::new(status_reporter_service));

	println!("Starting server on http://{:?}", addr);

	let server = hyper::Server::bind(&addr).serve(svc);

	server.await?;

	Ok(())
}
