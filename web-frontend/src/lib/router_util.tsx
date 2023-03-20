import { useSearchParams } from "@solidjs/router";

/** Get server from search params, set as server and clear server param*/
export function router_extract_server_url(): URL | undefined {
	const [params, setParams] = useSearchParams();
	const search = params["server"];
	if (!search) return undefined;
	setParams({server: undefined});
	return new URL(search);
}
