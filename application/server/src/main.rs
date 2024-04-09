use server::make_multiplexed_service;

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

	let make_multiplexer = make_multiplexed_service();
	println!("Starting server on http://{:?}", addr);
	let server = hyper::Server::bind(&addr).serve(make_multiplexer);
	let graceful = server.with_graceful_shutdown(shutdown_signal());

	graceful.await?;
	Ok(())
}
