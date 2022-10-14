//! Module to validate authentication to the grpc_interface

use std::str::FromStr;

use tonic::codegen::InterceptedService;
use tonic::{Request, Status};
use uuid::Uuid;

use grpc_proto::proto::segmented_encoder_server::SegmentedEncoderServer;

use crate::client_interface::grpc_service::auth_interceptor::AuthResult::{NoAuth, Successful};
use crate::client_interface::grpc_service::ServiceLock;

///Struct used to validate client credentials
pub(super) struct AuthenticationExtension(Option<Uuid>);

pub enum AuthResult {
	///Failed to verify credentials
	Err(Status),
	Successful(Uuid),
	NoAuth,
}

impl AuthResult {
	/// Returns the client if authentication was validated, or Err with [tonic::Status] describing
	/// the issue. Useful with `?` where authentication is required
	pub(crate) fn successful(self) -> Result<Uuid, Status> {
		match self {
			AuthResult::Err(status) => Err(status),
			Successful(uuid) => Ok(uuid),
			NoAuth => Err(Status::unauthenticated("Not authenticated")),
		}
	}
}

impl AuthenticationExtension {
	/// Validate client credentials
	async fn verify(&self, service: &ServiceLock) -> AuthResult {
		match self.0 {
			None => NoAuth,
			Some(uuid) => match service.0.read().await.get_client(&uuid) {
				None => AuthResult::Err(Status::unauthenticated("Unknown id")),
				Some(_) => Successful(uuid),
			},
		}
	}
	/// Calls verify on request's extension
	pub(crate) async fn verify_request<T>(req: &Request<T>, service: &ServiceLock) -> AuthResult {
		let extension = req.extensions().get::<AuthenticationExtension>();
		match extension {
			None => AuthResult::Err(Status::internal("Missing authentication extension")),
			Some(extension) => extension.verify(service).await,
		}
	}
}

type ServiceWithAuth = InterceptedService<
	SegmentedEncoderServer<ServiceLock>,
	fn(Request<()>) -> Result<Request<()>, Status>,
>;

impl ServiceLock {
	///Add authentication interceptor to service
	pub(crate) fn with_auth(self) -> ServiceWithAuth {
		SegmentedEncoderServer::with_interceptor(self, intercept_credentials)
	}
}

/// Inject [AuthenticationExtension] with parsed credentials on requests
fn intercept_credentials(mut request: Request<()>) -> Result<Request<()>, Status> {
	let worker_id = request
		.metadata()
		.get("worker-id")
		.map(|str| str.to_str().map(Uuid::from_str));
	let worker_id = worker_id
		.map(|a| match a {
			Ok(Ok(uuid)) => Ok(uuid),
			_ => Err(Status::unauthenticated("Invalid authentication")),
		})
		.transpose()?;
	request
		.extensions_mut()
		.insert(AuthenticationExtension(worker_id));
	Ok(request)
}
