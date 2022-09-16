use std::fs::canonicalize;
use std::process::Command;

fn main() -> Result<(), Box<dyn std::error::Error>> {
	// println!("cargo:rerun-if-changed=web-root"); //pack_web already does this
	let path = std::path::PathBuf::from("web-root");
	let path = canonicalize(path)?;
	let mut entry = path.clone();
	entry.push("index.ts");
	let mut out = path.clone();
	out.push("out.js");
	println!("Entry: {entry:?}, out:{out:?}");
	let result = Command::new("npx")
		.args([
			"esbuild".to_string(),
			format!("{}", entry.to_str().expect("Not UTF-8")),
			"--sourcemap".to_string(),
			"--bundle".to_string(),
			format!("--outfile={}", out.to_str().expect("Not UTF-8")),
		])
		.status()?
		.success();
	assert!(result, "Failed to bundle script!");

	web_packer::pack_web(&path)?;
	Ok(())
}
