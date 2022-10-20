use std::error::Error;
use std::future::Future;

use hyper::Uri;
use tonic::transport::Endpoint;
use tonic::Code;

use grpc_proto::proto::segmented_encoder_client::SegmentedEncoderClient;
use grpc_proto::proto::Empty;
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
