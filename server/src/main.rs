use std::net::SocketAddr;

use server::make_service;

async fn shutdown_signal() {
	// Wait for the CTRL+C signal
	tokio::signal::ctrl_c()
		.await
		.expect("failed to install CTRL+C signal handler");
	println!("Received CTRL+C");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	let addr = "[::]:8888".parse().unwrap();

	let web_service = make_service();
	let make_web = web_service.into_make_service_with_connect_info::<SocketAddr>();
	let make_grpc = grpc_proto::echo::service::shared();

	use multiplex_tonic_hyper::MakeMultiplexer;
	let make_multiplexer = MakeMultiplexer::new(make_grpc, make_web);
	println!("Starting server on http://{:?}", addr);
	let server = hyper::Server::bind(&addr).serve(make_multiplexer);
	let graceful = server.with_graceful_shutdown(shutdown_signal());

	graceful.await?;
	Ok(())
}
