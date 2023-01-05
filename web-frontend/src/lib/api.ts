function get_api_path() {
	return new URL("http://localhost:8888/api");
}

function get_path_on_api(path: string) {
	const url = get_api_path();
	url.pathname += path;
	console.log(url);
	return url;
}

export { get_api_path, get_path_on_api };