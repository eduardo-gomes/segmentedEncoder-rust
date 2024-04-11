async fn shutdown_signal() {
	// Wait for the CTRL+C signal
	tokio::signal::ctrl_c()
		.await
		.expect("failed to install CTRL+C signal handler");
	println!("Received CTRL+C");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	unimplemented!();
}
