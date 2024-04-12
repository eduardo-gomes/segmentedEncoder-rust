use std::net::SocketAddr;
use std::time::Duration;

use axum_server::Handle;

async fn shutdown_signal(handle: Handle) {
	// Wait for the CTRL+C signal
	tokio::signal::ctrl_c()
		.await
		.expect("failed to install CTRL+C signal handler");
	println!("Received CTRL+C");

	handle.graceful_shutdown(Some(Duration::from_secs(30)));
}

#[tokio::main]
async fn main() {
	let app = server::make_router(server::AppState::with_cred("password"));
	let handle = Handle::new();

	// Spawn a task to gracefully shutdown server.
	tokio::spawn(shutdown_signal(handle.clone()));

	let addr = SocketAddr::from(([127, 0, 0, 1], 8888));
	println!("listening on {}", addr);
	axum_server::bind(addr)
		.handle(handle)
		.serve(app.into_make_service())
		.await
		.unwrap();
}
