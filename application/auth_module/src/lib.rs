//! Authentication module
//!
//! This module will generate authentication tokens, and store permissions

use uuid::Uuid;

pub use local::LocalAuthenticator;

#[derive(Debug)]
pub enum Error {
	Error,
	InvalidCredentials,
}

pub trait AuthenticationHandler {
	async fn new_token(&self) -> String;
	async fn delete_token(&self, token: &str) -> Result<(), Error>;
	async fn add(&self, token: &str, obj: Uuid) -> Result<(), Error>;
	async fn remove(&self, token: &str, obj: Uuid) -> Result<bool, Error>;
	async fn check(&self, token: &str, obj: Uuid) -> Result<bool, Error>;
}

mod local {
	use std::collections::{HashMap, HashSet};
	use std::sync::atomic::Ordering;
	use std::sync::{RwLockReadGuard, RwLockWriteGuard};

	use uuid::Uuid;

	use crate::Error::InvalidCredentials;
	use crate::{AuthenticationHandler, Error};

	#[derive(Default)]
	pub struct LocalAuthenticator {
		counter: std::sync::atomic::AtomicUsize,
		map: std::sync::RwLock<HashMap<String, HashSet<Uuid>>>,
	}

	impl LocalAuthenticator {
		fn read_map(&self) -> RwLockReadGuard<'_, HashMap<String, HashSet<Uuid>>> {
			self.map.read().unwrap_or_else(|poison| poison.into_inner())
		}
		fn write_map(&self) -> RwLockWriteGuard<'_, HashMap<String, HashSet<Uuid>>> {
			self.map
				.write()
				.unwrap_or_else(|poison| poison.into_inner())
		}
	}

	impl AuthenticationHandler for LocalAuthenticator {
		async fn new_token(&self) -> String {
			let id = self.counter.fetch_add(1, Ordering::SeqCst);
			let token = id.to_string();
			self.write_map().insert(token.clone(), Default::default());
			token
		}

		async fn delete_token(&self, token: &str) -> Result<(), Error> {
			self.write_map()
				.remove(token)
				.map(|_| ())
				.ok_or(InvalidCredentials)
		}

		async fn add(&self, token: &str, obj: Uuid) -> Result<(), Error> {
			self.write_map()
				.get_mut(token)
				.ok_or(InvalidCredentials)?
				.insert(obj);
			Ok(())
		}

		async fn remove(&self, token: &str, obj: Uuid) -> Result<bool, Error> {
			match self.write_map().get_mut(token) {
				Some(perms) => Ok(perms.remove(&obj)),
				None => Err(InvalidCredentials),
			}
		}

		async fn check(&self, token: &str, obj: Uuid) -> Result<bool, Error> {
			Ok(self
				.read_map()
				.get(token)
				.and_then(|perms| perms.get(&obj))
				.is_some())
		}
	}

	#[cfg(test)]
	mod tests {
		use crate::Error::InvalidCredentials;

		use super::*;

		#[tokio::test]
		async fn authentication_create_different_tokens() {
			let handler = LocalAuthenticator::default();
			let token1 = handler.new_token().await;
			let token2 = handler.new_token().await;
			assert_ne!(token1, token2);
		}

		#[tokio::test]
		async fn check_new_token_returns_false() {
			let handler = LocalAuthenticator::default();
			let token = handler.new_token().await;
			let obj = Uuid::from_u64_pair(1, 2);
			let check = handler.check(token.as_str(), obj).await.unwrap();
			assert!(!check)
		}

		#[tokio::test]
		async fn add_invalid_token_errors() {
			let handler = LocalAuthenticator::default();
			let token = "Invalid_Token";
			let obj = Uuid::from_u64_pair(1, 2);
			let result = handler.add(token, obj).await.err().unwrap();
			assert!(matches!(result, InvalidCredentials))
		}

		#[tokio::test]
		async fn delete_invalid_token_errors() {
			let handler = LocalAuthenticator::default();
			let token = "Invalid_Token";
			let result = handler.delete_token(token).await.err().unwrap();
			assert!(matches!(result, InvalidCredentials))
		}

		#[tokio::test]
		async fn add_after_delete_token_errors() {
			let handler = LocalAuthenticator::default();
			let token = handler.new_token().await;
			handler.delete_token(token.as_str()).await.unwrap();
			let obj = Uuid::from_u64_pair(1, 2);
			let result = handler.add(token.as_str(), obj).await.err().unwrap();
			assert!(matches!(result, InvalidCredentials))
		}

		#[tokio::test]
		async fn remove_invalid_token_errors() {
			let handler = LocalAuthenticator::default();
			let token = "Invalid_Token";
			let obj = Uuid::from_u64_pair(1, 2);
			let result = handler.remove(token, obj).await.err().unwrap();
			assert!(matches!(result, InvalidCredentials))
		}

		#[tokio::test]
		async fn check_true_after_add() {
			let handler = LocalAuthenticator::default();
			let token = handler.new_token().await;
			let obj = Uuid::from_u64_pair(1, 2);
			handler.add(token.as_str(), obj).await.unwrap();
			let check = handler.check(token.as_str(), obj).await.unwrap();
			assert!(check)
		}

		#[tokio::test]
		async fn check_false_after_add_and_remove() {
			let handler = LocalAuthenticator::default();
			let token = handler.new_token().await;
			let obj = Uuid::from_u64_pair(1, 2);
			handler.add(token.as_str(), obj).await.unwrap();
			handler.remove(token.as_str(), obj).await.unwrap();
			let check = handler.check(token.as_str(), obj).await.unwrap();
			assert!(!check)
		}
	}
}
