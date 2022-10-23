use std::net::SocketAddr;
use std::sync::{Arc, Weak};

use axum::extract::connect_info::IntoMakeServiceWithConnectInfo;
use axum::Router;
use multiplex_tonic_hyper::MakeMultiplexer;
use tokio::sync::RwLock;
use tower::make::Shared;

use crate::client_interface::{ServiceLock, ServiceWithAuth};
use crate::job_manager::{JobManager, JobManagerLock};
use crate::storage::Storage;

#[allow(dead_code)] //until we use
mod client_interface;
pub mod web;

struct State {
	manager: RwLock<JobManager>,
	grpc: Weak<ServiceLock>,
}

impl State {
	fn new(manager: JobManagerLock, grpc: Weak<ServiceLock>) -> Arc<Self> {
		Arc::new(Self { manager, grpc })
	}
}

/// Temporary function to 'build' the service.
/// Will be replaced with a proper builder to set service proprieties.
pub fn make_multiplexed_service(
) -> MakeMultiplexer<Shared<ServiceWithAuth>, IntoMakeServiceWithConnectInfo<Router, SocketAddr>> {
	let storage = Storage::new().unwrap();
	let manager_lock = RwLock::new(JobManager::new(storage));
	use crate::client_interface::Service;
	let service_lock = Arc::new(Service::new().into_lock());
	let weak_service = Arc::downgrade(&service_lock);
	let grpc_service = service_lock.with_auth();

	let state = State::new(manager_lock, weak_service);
	let web = web::make_service(state).into_make_service_with_connect_info::<SocketAddr>();
	MakeMultiplexer::new(Shared::new(grpc_service), web)
}

#[allow(dead_code)]
mod storage;

mod jobs;

#[allow(dead_code)]
mod job_manager;

//Sample webm file, to use on tests
#[cfg(test)]
pub(crate) const WEBM_SAMPLE: [u8; 185] = [
	0x1a, 0x45, 0xdf, 0xa3, 0x40, 0x20, 0x42, 0x86, 0x81, 0x01, 0x42, 0xf7, 0x81, 0x01, 0x42, 0xf2,
	0x81, 0x04, 0x42, 0xf3, 0x81, 0x08, 0x42, 0x82, 0x40, 0x04, 0x77, 0x65, 0x62, 0x6d, 0x42, 0x87,
	0x81, 0x02, 0x42, 0x85, 0x81, 0x02, 0x18, 0x53, 0x80, 0x67, 0x40, 0x8d, 0x15, 0x49, 0xa9, 0x66,
	0x40, 0x28, 0x2a, 0xd7, 0xb1, 0x40, 0x03, 0x0f, 0x42, 0x40, 0x4d, 0x80, 0x40, 0x06, 0x77, 0x68,
	0x61, 0x6d, 0x6d, 0x79, 0x57, 0x41, 0x40, 0x06, 0x77, 0x68, 0x61, 0x6d, 0x6d, 0x79, 0x44, 0x89,
	0x40, 0x08, 0x40, 0x8f, 0x40, 0x00, 0x00, 0x00, 0x00, 0x00, 0x16, 0x54, 0xae, 0x6b, 0x40, 0x31,
	0xae, 0x40, 0x2e, 0xd7, 0x81, 0x01, 0x63, 0xc5, 0x81, 0x01, 0x9c, 0x81, 0x00, 0x22, 0xb5, 0x9c,
	0x40, 0x03, 0x75, 0x6e, 0x64, 0x86, 0x40, 0x05, 0x56, 0x5f, 0x56, 0x50, 0x38, 0x25, 0x86, 0x88,
	0x40, 0x03, 0x56, 0x50, 0x38, 0x83, 0x81, 0x01, 0xe0, 0x40, 0x06, 0xb0, 0x81, 0x08, 0xba, 0x81,
	0x08, 0x1f, 0x43, 0xb6, 0x75, 0x40, 0x22, 0xe7, 0x81, 0x00, 0xa3, 0x40, 0x1c, 0x81, 0x00, 0x00,
	0x80, 0x30, 0x01, 0x00, 0x9d, 0x01, 0x2a, 0x08, 0x00, 0x08, 0x00, 0x01, 0x40, 0x26, 0x25, 0xa4,
	0x00, 0x03, 0x70, 0x00, 0xfe, 0xfc, 0xf4, 0x00, 0x00,
];
