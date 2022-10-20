use std::error::Error;
use std::future::Future;

use hyper::Uri;
use tonic::transport::Endpoint;
use tonic::Code;
use uuid::Uuid;

use grpc_proto::proto::segmented_encoder_client::SegmentedEncoderClient;
use grpc_proto::proto::{Empty, RegistrationRequest};
use server::make_multiplexed_service;

async fn start_server(
) -> Result<(impl Future<Output = Result<(), Box<dyn Error>>>, Uri), Box<dyn Error>> {
	let service = make_multiplexed_service();
	let addr = "[::1]:0".parse().unwrap();
	let server = hyper::Server::bind(&addr).serve(service);
	let addr = server.local_addr();
	let (tx, rx) = tokio::sync::oneshot::channel::<()>();
	let graceful = server.with_graceful_shutdown(async {
		rx.await.ok();
	});
	let server_handle = tokio::spawn(graceful);
	let port = addr.port();
	let close = async move {
		tx.send(()).map_err(|_| "the receiver dropped")?;
		server_handle.await??;
		Ok(())
	};
	let url = Uri::builder()
		.scheme("http")
		.authority(format! {"[::1]:{port}"})
		.path_and_query("/")
		.build()?;
	Ok((close, url))
}

#[tokio::test]
async fn grpc_service_is_available_from_main_test_server() -> Result<(), Box<dyn Error>> {
	let (close, url) = start_server().await?;
	let endpoint: Endpoint = url.into();
	let mut client = SegmentedEncoderClient::connect(endpoint).await?;
	let response = client
		.get_worker_registration(Empty {})
		.await
		.expect_err("Should be an error!");
	assert_eq!(
		response.code(),
		Code::Unauthenticated,
		"The grpc service would return Unauthenticated"
	);
	close.await
}

///Test implies grpc_service (who knows all workers) is accessible through shared state on api router
#[tokio::test]
async fn api_status_contains_registered_worker_id() -> Result<(), Box<dyn Error>> {
	let (close, url) = start_server().await?;
	let worker_id = {
		let endpoint: Endpoint = url.clone().into();
		let mut client = SegmentedEncoderClient::connect(endpoint).await?;
		let response = client
			.register_client(RegistrationRequest {
				display_name: "Test client".to_string(),
			})
			.await?;
		Uuid::from_slice(response.into_inner().worker_id.as_slice())?
	};

	let url = url.to_string();
	let url: reqwest::Url = url.as_str().try_into()?;
	let url = url.join("/api/status")?;
	let response = reqwest::get(url).await?.text().await?;
	assert!(
		response.contains(&worker_id.as_hyphenated().to_string()),
		"'{response}' should contain the worker id '{worker_id}'"
	);
	close.await
}
