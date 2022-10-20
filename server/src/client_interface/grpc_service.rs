use tokio::sync::{RwLock, RwLockReadGuard};
use tonic::{Request, Response, Status};

use grpc_proto::proto::segmented_encoder_server::SegmentedEncoder;
use grpc_proto::proto::{Empty, RegistrationRequest, RegistrationResponse};

use crate::client_interface::grpc_service::auth_interceptor::AuthenticationExtension;

use super::Service;

pub(super) mod auth_interceptor;

pub struct ServiceLock(RwLock<Service>);

impl ServiceLock {
	pub(crate) async fn read(&self) -> RwLockReadGuard<'_, Service> {
		self.0.read().await
	}
}

impl Service {
	pub(crate) fn into_lock(self) -> ServiceLock {
		ServiceLock(RwLock::new(self))
	}
}

#[tonic::async_trait]
impl SegmentedEncoder for ServiceLock {
	async fn register_client(
		&self,
		_request: Request<RegistrationRequest>,
	) -> Result<Response<RegistrationResponse>, Status> {
		let (uuid, _) = self.0.write().await.register_client();
		Ok(Response::new(RegistrationResponse {
			worker_id: uuid.into_bytes().to_vec(),
		}))
	}

	async fn get_worker_registration(
		&self,
		request: Request<Empty>,
	) -> Result<Response<RegistrationResponse>, Status> {
		let worker_id = AuthenticationExtension::verify_request(&request, self)
			.await
			.successful()?;
		Ok(Response::new(RegistrationResponse {
			worker_id: worker_id.into_bytes().to_vec(),
		}))
	}
}

#[cfg(test)]
mod test {
	use std::error::Error;
	use std::future::Future;
	use std::str::FromStr;
	use std::sync::Arc;

	use tokio::sync::RwLock;
	use tonic::transport::{Channel, Endpoint};
	use tonic::{Code, Request};
	use tower::make::Shared;
	use uuid::Uuid;

	use grpc_proto::proto::segmented_encoder_client::SegmentedEncoderClient;
	use grpc_proto::proto::{client_with_auth, Empty, RegistrationRequest};

	use crate::client_interface::grpc_service::ServiceLock;
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
			.expect_err("Should not accept unauthenticated requests");
		assert_eq!(response.code(), Code::Unauthenticated);

		close.await?;
		Ok(())
	}

	#[tokio::test]
	async fn get_worker_registration_authenticated() -> Result<(), Box<dyn Error>> {
		let (close, mut client, url) = start_server().await?;
		let worker_id = register(&mut client).await?;
		let channel = Endpoint::from_str(&url)?.connect().await?;
		let mut client = client_with_auth(channel, worker_id);

		let response = client
			.get_worker_registration(Empty {})
			.await
			.expect("Should allow authenticated");
		let response = response.into_inner();
		assert_eq!(Uuid::from_slice(response.worker_id.as_slice())?, worker_id);

		close.await?;
		Ok(())
	}

	#[tokio::test]
	async fn random_uuid_is_not_authenticated() -> Result<(), Box<dyn Error>> {
		let (close, _, url) = start_server().await?;
		let channel = Endpoint::from_str(&url)?.connect().await?;
		let mut client =
			SegmentedEncoderClient::with_interceptor(channel, move |mut req: Request<()>| {
				req.metadata_mut()
					.insert("worker-id", Uuid::new_v4().to_string().parse().unwrap());
				Ok(req)
			});

		let response = client
			.get_worker_registration(Empty {})
			.await
			.expect_err("Should not authenticate");
		assert_eq!(response.code(), Code::Unauthenticated);

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
		let instance = Arc::new(ServiceLock(RwLock::new(Service::new())));
		let service = Shared::new(instance.with_auth());
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
