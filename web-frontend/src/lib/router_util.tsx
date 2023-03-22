import { useSearchParams } from "@solidjs/router";

/** Get server from search params and set as server*/
export function router_extract_server_url(): URL | undefined {
	const [params] = useSearchParams();
	const search = params["server"];
	if (!search) return undefined;
	return new URL(search);
}
