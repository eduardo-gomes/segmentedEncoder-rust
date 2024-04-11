use std::env;
use std::path::{Path, PathBuf};

use futures_util::TryStreamExt;
use tokio_util::io::StreamReader;

async fn download_cli() -> PathBuf {
	let jar = "https://repo1.maven.org/maven2/org/openapitools/openapi-generator-cli/7.2.0/openapi-generator-cli-7.2.0.jar";
	let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
	let out_file = out_dir.join("openapi-generator-cli.jar");
	let current_file = tokio::fs::metadata(&out_file)
		.await
		.map(|meta| meta.len())
		.ok();
	if current_file.is_some() {
		return out_file;
	}
	let res = reqwest::get(jar).await.unwrap();
	assert!(
		res.status().is_success(),
		"Should be able to download generator"
	);

	let mut stream = StreamReader::new(
		res.bytes_stream()
			.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e)),
	);
	let mut file = tokio::fs::File::create(&out_file).await.unwrap();
	tokio::io::copy(&mut stream, &mut file).await.unwrap();
	out_file
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
	println!("cargo::rerun-if-changed=../../api.yaml");
	let file = download_cli().await;
	println!("Downloaded to {:?}", file);
	let out_lib = Path::new(&env::var("OUT_DIR").unwrap()).join("generated");
	let status = std::process::Command::new("java")
		.arg("-jar")
		.arg(file)
		.args(["generate", "-i", "../../api.yaml", "-g", "rust", "-o"])
		.arg(out_lib)
		.status()
		.unwrap();
	if !status.success() {
		panic!("Failed to generate")
	}
}
