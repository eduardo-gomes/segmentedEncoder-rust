//! Module to store files
//!
//! Each file will be mapped to a UUID

#[cfg(test)]
mod test {
	#[test]
	fn create_file_returns_file_and_id() {
		let storage = Storage::new();
		let (file, uuid) = storage.create_file();
		assert!(false)
	}
}
