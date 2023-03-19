import { useSearchParams } from "@solidjs/router";
import { useContext } from "solid-js";
import { ApiContext } from "./api";

/** Get server from search params, set as server and clear server param*/
export function ApiExtractServer() {
	const [params, setParams] = useSearchParams();
	const search = params["server"];
	if (!search) return null;
	setParams({server: undefined});
	const url = new URL(search);
	const {set_url} = useContext(ApiContext);
	set_url(url);
	return null;
}
