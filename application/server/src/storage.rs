use tokio::io::{AsyncRead, AsyncSeek};
use uuid::Uuid;

mod old;

/// Trait for async file operations
///
/// Each file will be mapped to a UUID, and the related types supports streaming through AsyncRead and AsyncWrite
trait Storage {
	async fn read_file(&self, uuid: &Uuid) -> std::io::Result<impl AsyncRead + AsyncSeek>;
}

mod mem {
	//! Memory based storage
	use std::io::{Cursor, Error, ErrorKind};

	use tokio::io::{AsyncRead, AsyncSeek};
	use uuid::Uuid;

	use crate::storage::Storage;

	#[derive(Default)]
	struct MemStorage {}

	impl Storage for MemStorage {
		async fn read_file(&self, _uuid: &Uuid) -> std::io::Result<Cursor<Vec<u8>>> {
			Err(Error::new(ErrorKind::NotFound, "Not found"))
		}
	}

	#[cfg(test)]
	mod test {
		use std::io::ErrorKind;

		use uuid::Uuid;

		use crate::storage::mem::MemStorage;
		use crate::storage::Storage;

		#[tokio::test]
		async fn read_nonexistent_file_not_found() {
			let storage = MemStorage::default();
			let read = storage.read_file(&Uuid::nil()).await;
			assert!(read.is_err());
			assert_eq!(read.unwrap_err().kind(), ErrorKind::NotFound);
		}
	}
}
