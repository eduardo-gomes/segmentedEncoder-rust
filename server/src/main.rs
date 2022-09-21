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

	let web_service = make_service().into_make_service_with_connect_info::<SocketAddr>();

	println!("Starting server on http://{:?}", addr);
	let server = hyper::Server::bind(&addr).serve(web_service);
	let graceful = server.with_graceful_shutdown(shutdown_signal());

	graceful.await?;
	Ok(())
}
