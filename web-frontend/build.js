import { build } from "esbuild";
import fs from "fs";

const is_dev = process.argv.at(-1) === "dev";

build({
	entryPoints: ["./src/index.ts"],
	bundle: true,
	sourcemap: true,
	minify: false,
	format: "esm",
	outfile: "out/out.js",
	logLevel: "info",
	watch: is_dev,
}).catch(() => process.exit(1));

fs.mkdirSync("./out", {recursive: true});
fs.copyFileSync("./src/index.html", "./out/index.html");
fs.copyFileSync("./src/style.css", "./out/style.css");