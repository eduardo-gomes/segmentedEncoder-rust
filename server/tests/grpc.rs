use tonic::transport::Endpoint;
use tonic::Code;

use grpc_proto::proto::segmented_encoder_client::SegmentedEncoderClient;
use grpc_proto::proto::Empty;
use server::make_multiplexed_service;

#[tokio::test]
async fn grpc_service_is_available_from_main_test_server() -> Result<(), Box<dyn std::error::Error>>
{
	let server = make_multiplexed_service();
	let (url, handle, tx) = {
		let addr = "[::1]:0".parse()?;
		let server = hyper::Server::bind(&addr).serve(server);
		let addr = server.local_addr();
		let (tx, rx) = tokio::sync::oneshot::channel::<()>();
		let graceful = server.with_graceful_shutdown(async {
			rx.await.ok();
		});
		let server_handle = tokio::spawn(graceful);
		let port = addr.port();
		let url = format!("http://[::1]:{port}");
		(url, server_handle, tx)
	};
	let endpoint: Endpoint = url.parse()?;
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
	tx.send(()).map_err(|_| "the receiver dropped")?;
	Ok(handle.await??)
}
