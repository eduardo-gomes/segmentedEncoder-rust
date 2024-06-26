use std::future::Future;

use tokio::io::{AsyncRead, AsyncSeek, AsyncWrite};
use uuid::Uuid;

pub(crate) use mem::MemStorage;

/// Trait for async file operations
///
/// Each file will be mapped to a UUID, and the related types supports streaming through AsyncRead and AsyncWrite
pub trait Storage: Sync {
	type WriteFile: AsyncWrite + Send + Unpin;
	fn read_file(
		&self,
		uuid: Uuid,
	) -> impl Future<Output = std::io::Result<impl AsyncRead + AsyncSeek + Send + Unpin + 'static>> + Send;
	///Create a writer for a new file, the content may only be stored after a call to store
	fn create_file(&self) -> impl Future<Output = std::io::Result<Self::WriteFile>> + Send;
	///Save the file and return its id
	fn store_file(
		&self,
		file: Self::WriteFile,
	) -> impl Future<Output = std::io::Result<Uuid>> + Send;
	///Copy the body content to a new file
	fn body_to_new_file(
		&self,
		body: axum::body::Body,
	) -> impl Future<Output = std::io::Result<Uuid>> + Send
	where
		Self: Sync,
	{
		use futures::StreamExt;
		use std::io;
		use std::io::ErrorKind;
		use tokio_util::io::StreamReader;
		async {
			let mut write = self.create_file().await?;
			let body_stream = body
				.into_data_stream()
				.map(|res| res.map_err(|e| io::Error::new(ErrorKind::Other, e)));
			let mut stream = StreamReader::new(body_stream);
			tokio::io::copy(&mut stream, &mut write).await?;
			self.store_file(write).await
		}
	}
}

mod mem {
	//! Memory based storage
	use std::collections::BTreeMap;
	use std::fmt::{Debug, Formatter};
	use std::io::{Cursor, Error, ErrorKind};
	use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

	use uuid::Uuid;

	use crate::storage::Storage;

	#[derive(Default)]
	pub(crate) struct MemStorage {
		storage: RwLock<BTreeMap<Uuid, MemReadFile>>,
	}

	impl MemStorage {
		fn read(&self) -> RwLockReadGuard<'_, BTreeMap<Uuid, MemReadFile>> {
			self.storage
				.read()
				.unwrap_or_else(|poison| poison.into_inner())
		}
		fn write(&self) -> RwLockWriteGuard<'_, BTreeMap<Uuid, MemReadFile>> {
			self.storage
				.write()
				.unwrap_or_else(|poison| poison.into_inner())
		}
	}

	#[derive(Clone)]
	pub(crate) struct MemReadFile(Arc<Vec<u8>>);

	impl Debug for MemReadFile {
		fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
			f.debug_struct("MemReadFile")
				.field("len", &self.0.len())
				.finish()
		}
	}

	impl AsRef<[u8]> for MemReadFile {
		fn as_ref(&self) -> &[u8] {
			&self.0
		}
	}

	impl Storage for MemStorage {
		type WriteFile = Vec<u8>;

		async fn read_file(&self, uuid: Uuid) -> std::io::Result<Cursor<MemReadFile>> {
			self.read()
				.get(&uuid)
				.cloned()
				.map(Cursor::new)
				.ok_or(Error::new(ErrorKind::NotFound, "Not found"))
		}

		async fn create_file(&self) -> std::io::Result<Self::WriteFile> {
			Ok(Vec::new())
		}

		async fn store_file(&self, file: Self::WriteFile) -> std::io::Result<Uuid> {
			let id = Uuid::new_v4();
			self.write().insert(id, MemReadFile(Arc::new(file)));
			Ok(id)
		}
	}

	#[cfg(test)]
	mod test {
		use std::io::ErrorKind;

		use tokio::io::{AsyncReadExt, AsyncWrite, AsyncWriteExt};
		use uuid::Uuid;

		use crate::storage::mem::MemStorage;
		use crate::storage::Storage;
		use crate::MKV_SAMPLE;

		#[tokio::test]
		async fn read_nonexistent_file_not_found() {
			let storage = MemStorage::default();
			let read = storage.read_file(Uuid::nil()).await;
			assert!(read.is_err());
			assert_eq!(read.unwrap_err().kind(), ErrorKind::NotFound);
		}

		#[tokio::test]
		async fn create_file_return_write() {
			let storage = MemStorage::default();
			let write: Result<Box<dyn AsyncWrite>, _> = storage
				.create_file()
				.await
				.map(|v| Box::new(v) as Box<dyn AsyncWrite>);
			assert!(write.is_ok());
		}

		#[tokio::test]
		async fn create_file_the_store_returns_uuid() {
			let storage = MemStorage::default();
			let write = storage.create_file().await.unwrap();
			let id = storage.store_file(write).await;
			assert!(id.is_ok())
		}

		#[tokio::test]
		async fn create_file_the_store_returns_non_nil_uuid() {
			let storage = MemStorage::default();
			let write = storage.create_file().await.unwrap();
			let id = storage.store_file(write).await.unwrap();
			assert!(!id.is_nil())
		}

		#[tokio::test]
		async fn read_file_with_uuid_from_store_ok() {
			let storage = MemStorage::default();
			let write = storage.create_file().await.unwrap();
			let id = storage.store_file(write).await.unwrap();
			let read = storage.read_file(id).await;
			assert!(read.is_ok())
		}

		#[tokio::test]
		async fn read_file_has_written_content() {
			let storage = MemStorage::default();
			let mut write = storage.create_file().await.unwrap();
			let input = &MKV_SAMPLE;
			AsyncWriteExt::write_all(&mut write, input).await.unwrap();
			let id = storage.store_file(write).await.unwrap();
			let mut read = storage.read_file(id).await.unwrap();
			let mut out = Vec::new();
			AsyncReadExt::read_to_end(&mut read, &mut out)
				.await
				.unwrap();
			assert_eq!(out, input)
		}
	}
}
