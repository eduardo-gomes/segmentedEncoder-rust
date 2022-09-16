pub use bundle_web::bundle_web_root as pack_web;
pub use router::gen_router_from_web_root;

///This macro expands to an instance of axum::Router with routes to the files from the directory
/// specified on the build script
#[macro_export]
macro_rules! include_web_static {
	() => {{
		mod map {
			include!(concat!(env!("OUT_DIR"), concat!("/bundled_web.rs")));
		}
		let router = web_packer::gen_router_from_web_root(map::get_map());
		router
	}};
}

mod router {
	use std::collections::HashMap;

	use axum::routing::get;
	use axum::Router;

	fn gen_response(path: &str, content: &'static [u8]) -> hyper::Response<hyper::Body> {
		use axum::http::header::CONTENT_TYPE;
		use hyper::Body;
		use hyper::Response;
		use std::path::Path;
		let ext = Path::new(path)
			.extension()
			.map(|e| e.to_str())
			.unwrap_or_default()
			.unwrap_or_default();
		let content_type = mime_guess::from_ext(ext).first_or(mime::TEXT_PLAIN_UTF_8);
		Response::builder()
			.header(CONTENT_TYPE, content_type.essence_str())
			.body(Body::from(content))
			.unwrap()
	}

	pub fn gen_router_from_web_root(map: HashMap<&'static str, &'static [u8]>) -> Router {
		let mut router = Router::new();
		for (path, content) in map {
			let handle = || async { gen_response(path, content) };
			router = router.route(path, get(handle));
		}
		router
	}
}

mod bundle_web {
	use std::ops::Add;
	use std::path::{Path, PathBuf};
	use std::{env, fs};

	///Generate a rust file that read the files at compile time, and generate a hashmap
	/// with the relative path as the key
	pub fn bundle_web_root(web_root: &Path) -> Result<(), Box<dyn std::error::Error>> {
		println!(
			"cargo:rerun-if-changed={}",
			web_root.to_str().ok_or("web_root is not valid UTF-8")?
		);
		let files = list_all_files(web_root)?;
		let out_dir = env::var_os("OUT_DIR")
			.ok_or("OUT_DIR is not set. Is this running in the build script?")?;
		let dest = Path::new(&out_dir).join("bundled_web.rs");

		let mut list = String::new();
		for (relative, absolute) in files {
			let line = format!(
				"\tmap.insert(\"{}\", include_bytes!(\"{}\").as_slice());\n",
				relative.to_str().ok_or("path is not valid UTF-8")?,
				absolute.to_str().ok_or("path is not valid UTF-8")?
			);
			println!("Mapping {relative:?} to {absolute:?}");
			list = list.add(&line);
		}

		let code = format!(
			"use std::collections::HashMap;\n\
			pub fn get_map() -> HashMap<&'static str, &'static [u8]>{{\n\
			\tlet mut map = HashMap::new();\n\
			{list}\
			\tmap\
			\n}}"
		);
		fs::write(dest, code)?;
		Ok(())
	}

	fn list_all_files(dir: &Path) -> Result<Vec<(PathBuf, PathBuf)>, std::io::Error> {
		let package_root = env::var("CARGO_MANIFEST_DIR").unwrap();
		let absolute_path = Path::new(&package_root).join(dir);
		let mut files = Vec::new();
		recursive_list_all_files(&absolute_path, Path::new("/"), &mut files)?;
		Ok(files)
	}

	fn recursive_list_all_files(
		dir: &Path,
		relative: &Path,
		files: &mut Vec<(PathBuf, PathBuf)>,
	) -> Result<(), std::io::Error> {
		if dir.is_dir() {
			for entry in fs::read_dir(dir)? {
				let entry = entry?;
				let path = entry.path();
				let rel = path.strip_prefix(dir).expect("Failed to get relative path");
				let rel = relative.clone().join(rel);
				if path.is_dir() {
					recursive_list_all_files(&path, &rel, files)?;
				} else {
					files.push((rel.to_path_buf(), path));
				}
			}
		}
		Ok(())
	}
}
