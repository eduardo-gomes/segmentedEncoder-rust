use std::env;
use std::fs::canonicalize;
use std::io::Error;
use std::path::{Path, PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
	println!("cargo:rerun-if-changed=web-src");
	let path = PathBuf::from("web-src");
	let path = canonicalize(path)?;
	let entry = path.join("index.ts");
	let out_filename = PathBuf::from("out.js");
	let static_path = PathBuf::from("web-root");

	let result = build(&entry, &out_filename)
		.expect("Could not run esbuild!")
		.expect("Build failed, check source!");
	web_packer::pack_web(&static_path, Some(&result)).unwrap();
	Ok(())
}

fn build(entry: &Path, out_filename: &PathBuf) -> Result<Option<PathBuf>, Error> {
	let generated_path = PathBuf::from(env::var("OUT_DIR").unwrap()).join("bundled_js");
	let out_path = generated_path.join(out_filename);

	println!("Entry: {entry:?}, out:{out_filename:?}, out_dir:{generated_path:?}");
	let out_file_arg = format!("--outfile={}", out_path.to_str().expect("Not UTF-8"));
	let args = [
		entry.as_os_str(),
		"--sourcemap".as_ref(),
		"--bundle".as_ref(),
		out_file_arg.as_ref(),
	];

	web_packer::esbuild::run(args.as_slice()).map(|status| status.then_some(generated_path))
}
