use std::env;
use std::fs::canonicalize;
use std::path::PathBuf;
use std::process::Command;

#[cfg(not(windows))]
const NPX: &str = "npx";
#[cfg(windows)]
const NPX: &str = "npx.cmd";

fn main() -> Result<(), Box<dyn std::error::Error>> {
	let generated_path = PathBuf::from(env::var("OUT_DIR").unwrap()).join("bundled_js");

	println!("cargo:rerun-if-changed=web-src");
	let path = std::path::PathBuf::from("web-src");
	let path = canonicalize(path)?;
	let entry = path.join("index.ts");
	let out = generated_path.join("out.js");
	println!("Entry: {entry:?}, out:{out:?}");
	let result = Command::new(NPX)
		.args([
			"esbuild".to_string(),
			entry.to_str().expect("Not UTF-8").to_string(),
			"--sourcemap".to_string(),
			"--bundle".to_string(),
			format!("--outfile={}", out.to_str().expect("Not UTF-8")),
		])
		.status()?
		.success();
	assert!(result, "Failed to bundle script!");

	let static_path = std::path::PathBuf::from("web-root");
	web_packer::pack_web(&static_path, Some(&generated_path))?;
	Ok(())
}
