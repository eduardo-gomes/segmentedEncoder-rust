/** Get server from search params and set as server*/
export function router_extract_server_url(): URL | undefined {
	const params = new URLSearchParams(location.hash.substring(1));
	const search = params.get("server");
	if (!search) return undefined;
	return new URL(search);
}
