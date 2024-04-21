use std::net::SocketAddr;
use std::time::Duration;

use axum::http::{HeaderName, HeaderValue};
use axum::routing::Router;
use axum_server::Handle;
use clap::Parser;

async fn shutdown_signal(handle: Handle) {
	// Wait for the CTRL+C signal
	tokio::signal::ctrl_c()
		.await
		.expect("failed to install CTRL+C signal handler");
	println!("Received CTRL+C");

	handle.graceful_shutdown(Some(Duration::from_secs(30)));
}

#[derive(Parser, Debug)]
struct Args {
	#[arg(short, long)]
	cors_origin: Vec<String>,
	#[arg(short, long, default_value = "password")]
	password: String,
}

#[tokio::main]
async fn main() {
	let args = Args::parse();
	let api = server::make_router(server::AppStateLocal::with_cred(&args.password).into());
	let origins: Vec<HeaderValue> = args
		.cors_origin
		.iter()
		.map(|val| val.parse().unwrap())
		.collect();
	let headers = [
		HeaderName::from_static("credentials"),
		HeaderName::from_static("audio_codec"),
		HeaderName::from_static("audio_param"),
		HeaderName::from_static("authorization"),
		HeaderName::from_static("content-type"),
		HeaderName::from_static("segment_duration"),
		HeaderName::from_static("video_codec"),
		HeaderName::from_static("video_param"),
	];
	let cors = tower_http::cors::CorsLayer::new()
		.allow_origin(origins)
		.allow_headers(headers)
		.allow_credentials(true);
	let app = Router::new().nest("/api", api).layer(cors);
	let handle = Handle::new();

	// Spawn a task to gracefully shutdown server.
	tokio::spawn(shutdown_signal(handle.clone()));

	let addr: SocketAddr = "[::]:8888".parse().unwrap();
	println!("listening on {}", addr);
	axum_server::bind(addr)
		.handle(handle)
		.serve(app.into_make_service())
		.await
		.unwrap();
}
