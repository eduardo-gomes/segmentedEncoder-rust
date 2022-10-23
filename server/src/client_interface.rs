//!This module implements the client interface, and will implement the grpc interface

use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::future::Future;
use std::sync::{Arc, Weak};

use uuid::Uuid;

pub(crate) use grpc_service::auth_interceptor::ServiceWithAuth;
pub(crate) use grpc_service::ServiceLock;

use crate::jobs::Task;
use crate::State;

type ClientEntry = Arc<()>;

mod grpc_service;

pub(crate) struct Service {
	///The Uuid is the client id. The access token will be stored(~~when implemented~~) on the map
	/// and should be verified before external access.
	clients: HashMap<Uuid, Arc<()>>,
	state: Weak<State>,
}

impl Service {
	pub(crate) fn with_state(mut self, state: Weak<State>) -> Self {
		self.state = state;
		self
	}
}

impl Debug for Service {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		let map: Vec<(String, &ClientEntry)> = self
			.clients
			.iter()
			.map(|(key, el)| (key.as_hyphenated().to_string(), el))
			.collect();
		f.debug_struct("Service").field("clients", &map).finish()
	}
}

impl Service {
	pub(crate) fn status(&self) -> String {
		format!("{self:#?}")
	}

	pub(crate) fn erase_client(&mut self, id: &Uuid) {
		self.clients.remove(id);
	}

	pub(crate) fn get_client(&self, id: &Uuid) -> Option<ClientEntry> {
		self.clients.get(id).cloned()
	}

	pub(crate) fn client_count(&self) -> usize {
		self.clients.len()
	}

	pub(crate) fn register_client(&mut self) -> (Uuid, ClientEntry) {
		let id = Uuid::new_v4();
		let arc = Arc::new(());
		self.clients.insert(id, arc.clone());
		(id, arc)
	}

	pub(crate) fn request_task(&self) -> Result<impl Future<Output = Option<Task>>, &'static str> {
		//The future owns the upgraded arc, write may be locked outside the ServiceLock
		self.state
			.upgrade()
			.ok_or("Service was dropped!")
			.map(|service| async move { service.manager.write().await.allocate() })
	}

	pub(crate) fn new() -> Self {
		Self {
			clients: HashMap::new(),
			state: Weak::default(),
		}
	}
}

#[cfg(test)]
mod test {
	use std::sync::Arc;

	use tokio::sync::RwLock;
	use uuid::Uuid;

	use crate::client_interface::Service;
	use crate::jobs::{Job, JobParams, Source};
	use crate::{JobManager, State, Storage};

	#[test]
	fn new_service_has_no_clients() {
		let service = Service::new();
		assert_eq!(service.client_count(), 0);
	}

	#[test]
	fn register_client_increment_client_count() {
		let mut service = Service::new();
		service.register_client();
		assert_eq!(service.client_count(), 1);
		service.register_client();
		assert_eq!(service.client_count(), 2);
	}

	#[test]
	fn get_client_id_return_same_object_than_register() {
		let mut service = Service::new();
		let (id, client) = service.register_client();

		let got = service.get_client(&id).unwrap();
		assert!(Arc::ptr_eq(&got, &client));
	}

	#[test]
	fn two_clients_are_different() {
		let mut service = Service::new();
		let (_id, client_1) = service.register_client();
		let (_id, client_2) = service.register_client();
		assert!(!Arc::ptr_eq(&client_1, &client_2));
	}

	#[test]
	fn remove_client_decrement_count() {
		let mut service = Service::new();
		let (id, _) = service.register_client();
		let (_id, _) = service.register_client();
		service.erase_client(&id);
		assert_eq!(service.client_count(), 1);
	}

	#[test]
	fn remove_two_times() {
		let mut service = Service::new();
		let (id, _) = service.register_client();
		let (_id, _) = service.register_client();
		service.erase_client(&id);
		service.erase_client(&id);
		assert_eq!(service.client_count(), 1);
	}

	#[test]
	fn get_after_remove_returns_none() {
		let mut service = Service::new();
		let (id, _) = service.register_client();
		service.erase_client(&id);
		let res = service.get_client(&id);
		assert!(res.is_none());
	}

	#[test]
	fn status_has_registered_workers_id() {
		let mut service = Service::new();
		let (id, _) = service.register_client();
		let status = service.status();
		assert!(
			status.contains(&id.as_hyphenated().to_string()),
			"Status '{status}' should contain worker_id '{id}'"
		);
	}

	#[tokio::test]
	async fn request_task_returns_none() {
		let manager_lock = {
			let manager = JobManager::new(Storage::new().unwrap());
			RwLock::new(manager)
		};
		let service = Service::new().into_lock();
		let state = State::new(manager_lock, service);
		let service = state.grpc.clone();
		let task = service.read().await.request_task().unwrap().await;
		assert!(task.is_none());
	}

	#[tokio::test]
	async fn request_task_returns_some_after_create_job() {
		let manager_lock = {
			let mut manager = JobManager::new(Storage::new().unwrap());
			manager.add_job(Job::new(
				Source::Local(Uuid::new_v4()),
				JobParams::sample_params(),
			));
			RwLock::new(manager)
		};

		let service = Service::new().into_lock();
		let state = State::new(manager_lock, service);
		let service = state.grpc.clone();
		let task = service.read().await.request_task().unwrap().await;
		assert!(task.is_some());
	}
}
