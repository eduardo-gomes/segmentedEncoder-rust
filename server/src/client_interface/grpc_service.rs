use std::str::FromStr;

use tonic::{Request, Response, Status};
use uuid::Uuid;

use grpc_proto::proto::segmented_encoder_server::SegmentedEncoder;
use grpc_proto::proto::{Empty, RegistrationRequest, RegistrationResponse};

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

	async fn get_worker_registration(
		&self,
		request: Request<Empty>,
	) -> Result<Response<RegistrationResponse>, Status> {
		let worker_id = request
			.metadata()
			.get("worker-id")
			.map(|str| str.to_str().map(|str| Uuid::from_str(str)));
		let worker_id = match worker_id {
			Some(Ok(Ok(a))) => Some(a),
			_ => None,
		};
		match worker_id {
			None => Err(Status::unauthenticated("Not authenticated")),
			Some(worker_id) => Ok(Response::new(RegistrationResponse {
				worker_id: worker_id.into_bytes().to_vec(),
			})),
		}
	}
}

#[cfg(test)]
mod test {
	use std::error::Error;
	use std::future::Future;
	use std::str::FromStr;

	use tonic::transport::{Channel, Endpoint};
	use tonic::{Code, Request};
	use tower::make::Shared;
	use uuid::Uuid;

	use grpc_proto::proto::segmented_encoder_client::SegmentedEncoderClient;
	use grpc_proto::proto::segmented_encoder_server::SegmentedEncoderServer;
	use grpc_proto::proto::{Empty, RegistrationRequest};

	use crate::client_interface::Service;

	#[tokio::test]
	async fn register_client_returns_uuid() -> Result<(), Box<dyn Error>> {
		let (close, mut client, _) = start_server().await?;

		let request = Request::new(RegistrationRequest {
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

	#[tokio::test]
	async fn get_worker_registration_needs_authentication() -> Result<(), Box<dyn Error>> {
		let (close, mut client, _) = start_server().await?;

		let response = client
			.get_worker_registration(Empty {})
			.await
			.err()
			.expect("Should not accept unauthenticated requests");
		assert_eq!(response.code(), Code::Unauthenticated);

		close.await?;
		Ok(())
	}

	#[tokio::test]
	async fn get_worker_registration_authenticated() -> Result<(), Box<dyn Error>> {
		let (close, mut client, url) = start_server().await?;
		let worker_id = register(&mut client).await?;
		let channel = Endpoint::from_str(&url)?.connect().await?;
		let mut client =
			SegmentedEncoderClient::with_interceptor(channel, move |mut req: Request<()>| {
				req.metadata_mut()
					.insert("worker-id", worker_id.to_string().parse().unwrap());
				Ok(req)
			});

		let response = client
			.get_worker_registration(Empty {})
			.await
			.expect("Should allow authenticated");
		let response = response.into_inner();
		assert_eq!(Uuid::from_slice(response.worker_id.as_slice())?, worker_id);

		close.await?;
		Ok(())
	}

	//Request registration and return the worker_id
	async fn register(
		client: &mut SegmentedEncoderClient<Channel>,
	) -> Result<Uuid, Box<dyn Error>> {
		let request = Request::new(RegistrationRequest {
			display_name: "Test worker".to_string(),
		});
		let response = client.register_client(request).await?;
		let worker_id = response.into_inner().worker_id;
		let worker_id = Uuid::from_slice(&worker_id)?;
		Ok(worker_id)
	}

	//Start the server and return future to close, a client and the url
	async fn start_server() -> Result<
		(
			impl Future<Output = Result<(), Box<dyn Error>>>,
			SegmentedEncoderClient<Channel>,
			String,
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
		let port = addr.port();
		let url = format!("http://[::1]:{port}");
		// println!("Connecting to {url}");

		let close = async move {
			tx.send(()).map_err(|_| "the receiver dropped")?;
			server_handle.await??;
			Ok(())
		};
		let client = grpc_proto::proto::segmented_encoder_client::SegmentedEncoderClient::connect(
			url.clone(),
		)
		.await?;
		Ok((close, client, url))
	}
}
