use tonic::{Request, Response, Status};
use uuid::Uuid;

use grpc_proto::proto::segmented_encoder_server::SegmentedEncoder;
use grpc_proto::proto::{RegistrationRequest, RegistrationResponse};

use super::Service;

#[tonic::async_trait]
impl SegmentedEncoder for Service {
	async fn register_client(
		&self,
		_request: Request<RegistrationRequest>,
	) -> Result<Response<RegistrationResponse>, Status> {
		Ok(Response::new(RegistrationResponse {
			worker_id: Uuid::new_v4().into_bytes().to_vec(),
		}))
	}
}

#[cfg(test)]
mod test {
	use std::error::Error;
	use std::future::Future;

	use tonic::transport::Channel;
	use tower::make::Shared;
	use uuid::Uuid;

	use grpc_proto::proto::segmented_encoder_client::SegmentedEncoderClient;
	use grpc_proto::proto::segmented_encoder_server::SegmentedEncoderServer;
	use grpc_proto::proto::RegistrationRequest;

	use crate::client_interface::Service;

	#[tokio::test]
	async fn register_client_returns_uuid() -> Result<(), Box<dyn Error>> {
		let (close, mut client) = start_server().await?;

		let request = tonic::Request::new(RegistrationRequest {
			display_name: "Test worker".to_string(),
		});

		let response = client.register_client(request).await?;
		let worker_id = response.into_inner().worker_id;
		let worker_id = Uuid::from_slice(&worker_id).expect("worker_id is not an uuid");
		assert!(!worker_id.is_nil(), "worker_id should not be null");

		//shutdown server
		close.await?;
		Ok(())
	}

	//Function to start the server and return future to close and the url
	async fn start_server() -> Result<
		(
			impl Future<Output = Result<(), Box<dyn Error>>>,
			SegmentedEncoderClient<Channel>,
		),
		Box<dyn Error>,
	> {
		let instance = Service::new();
		let service = Shared::new(SegmentedEncoderServer::new(instance));
		let addr = "[::1]:0".parse().unwrap();
		let server = hyper::Server::bind(&addr).serve(service);
		let addr = server.local_addr();
		let (tx, rx) = tokio::sync::oneshot::channel::<()>();
		let graceful = server.with_graceful_shutdown(async {
			rx.await.ok();
		});
		let server_handle = tokio::spawn(graceful);
		let url = format!("http://[::1]:{}", addr.port());
		// println!("Connecting to {url}");

		let close = async move {
			tx.send(()).map_err(|_| "the receiver dropped")?;
			server_handle.await??;
			Ok(())
		};
		let client =
			grpc_proto::proto::segmented_encoder_client::SegmentedEncoderClient::connect(url)
				.await?;
		Ok((close, client))
	}
}
