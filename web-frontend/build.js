import { build } from "esbuild";
import { solidPlugin } from "esbuild-plugin-solid";
import fs from "fs";

const is_dev = process.argv.at(-1) === "dev";

build({
	entryPoints: ["./src/index.tsx"],
	bundle: true,
	sourcemap: true,
	minify: false,
	format: "esm",
	outfile: "out/out.js",
	logLevel: "info",
	watch: is_dev,
	plugins: [solidPlugin()]
}).catch(() => process.exit(1));

fs.mkdirSync("./out", {recursive: true});
fs.copyFileSync("./src/index.html", "./out/index.html");
