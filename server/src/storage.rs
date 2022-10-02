//! Module to store files
//!
//! Each file will be mapped to a UUID

use std::io;
use std::path::PathBuf;

use tempfile::{tempdir, TempDir};
use tokio::fs::File;
use uuid::Uuid;

pub struct Storage {
	dir: TempDir,
}

impl Storage {
	pub(crate) async fn get_file(&self, uuid: &Uuid) -> io::Result<File> {
		let path = self.get_file_path(uuid);
		File::open(path).await
	}
	pub(crate) async fn create_file(&self) -> io::Result<(File, Uuid)> {
		let uuid = Uuid::new_v4();
		let path = self.get_file_path(&uuid);
		let file = File::create(path).await?;
		Ok((file, uuid))
	}

	fn get_file_path(&self, uuid: &Uuid) -> PathBuf {
		let path = self.dir.path();
		let path = path.join(uuid.as_simple().to_string());
		path
	}
}

impl Storage {
	pub(crate) fn new() -> io::Result<Self> {
		let dir = tempdir()?;
		Ok(Storage { dir })
	}
}

#[cfg(test)]
mod test {
	use std::io::Write;

	use tokio::io::AsyncReadExt;
	use uuid::Uuid;

	use crate::storage::Storage;

	#[tokio::test]
	async fn create_file_returns_file_and_id() {
		let storage = Storage::new().unwrap();
		let (_file, _uuid) = storage.create_file().await.unwrap();
	}

	#[tokio::test]
	async fn retrieve_nonexistent_file_fails() {
		let storage = Storage::new().unwrap();
		let random = Uuid::new_v4();
		let result = storage.get_file(&random).await;
		assert!(result.is_err())
	}

	#[tokio::test]
	async fn write_to_file_and_retrieve_using_uuid() {
		let random_data = Uuid::new_v4();
		let data = random_data.as_hyphenated().to_string();

		let storage = Storage::new().unwrap();
		let (file, uuid) = storage.create_file().await.unwrap();

		//Write random data to file
		let mut file = file.try_into_std().unwrap(); //So we dont need async in the test
		file.write(data.as_bytes()).unwrap();
		drop(file);

		let mut file = storage.get_file(&uuid).await.unwrap();
		let mut content = String::new();
		file.read_to_string(&mut content).await.unwrap();

		assert_eq!(content, data, "Should have the data we wrote before!");
	}
}

pub mod stream {
	use hyper::Body;
	use tokio::fs::File;
	use tokio::io::AsyncRead;
	use tokio_util::io::{ReaderStream, StreamReader};

	pub(crate) async fn body_to_file(body: Body, file: &mut File) -> std::io::Result<u64> {
		use futures::StreamExt;
		let body = body.map(|res| {
			res.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
		});
		let mut stream = StreamReader::new(body);
		tokio::io::copy(&mut stream, file).await
	}

	pub(crate) fn read_to_stream<T: AsyncRead>(read: T) -> ReaderStream<T> {
		ReaderStream::new(read)
	}

	#[cfg(test)]
	mod test {
		use hyper::Body;

		use crate::storage::stream::body_to_file;
		use crate::{Storage, WEBM_SAMPLE};

		#[tokio::test]
		async fn body_to_job_source() -> std::io::Result<()> {
			let body = Body::from(WEBM_SAMPLE.as_slice());

			let storage = Storage::new()?;
			let uuid = {
				let (mut file, uuid) = storage.create_file().await?;
				let _len = body_to_file(body, &mut file).await?;
				uuid
			};

			let mut file = storage.get_file(&uuid).await?;
			let mut read = Vec::new();
			use tokio::io::AsyncReadExt;
			file.read_to_end(&mut read).await?;
			assert_eq!(read, WEBM_SAMPLE, "Content should be the same");
			Ok(())
		}
	}
}
