//!This module implements the client interface, and will implement the grpc interface

use std::collections::HashMap;
use std::sync::Arc;

use uuid::Uuid;

pub(crate) use grpc_service::auth_interceptor::ServiceWithAuth;

type ClientEntry = Arc<()>;

mod grpc_service;

pub(crate) struct Service {
	///The Uuid is the client id. The access token will be stored(~~when implemented~~) on the map
	/// and should be verified before external access.
	clients: HashMap<Uuid, Arc<()>>,
}

impl Service {
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

	pub(crate) fn new() -> Self {
		Self {
			clients: HashMap::new(),
		}
	}
}

#[cfg(test)]
mod test {
	use std::sync::Arc;

	use crate::client_interface::Service;

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
}
