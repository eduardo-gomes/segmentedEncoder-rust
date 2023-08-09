import type {Accessor, ParentProps, Setter,} from "solid-js";
import {createContext, createEffect, createSignal, onCleanup} from "solid-js";
import {router_extract_server_url} from "./router_util";

function get_path_on_api(url: URL, path: string) {
	url.pathname += path;
	console.debug("Get path to", path, url);
	return url;
}

export type ApiContextType = {
	url: Accessor<URL>,
	/**Return server version string if is connected or undefined*/
	version: Accessor<string | undefined>,
	is_connected: Accessor<boolean>,
	set_url: Setter<URL>
	path_on_url: (path: string) => URL
};

const fallback_url = new URL("http://localhost:8888/api");

export const ApiContext = createContext<ApiContextType>({
	url: () => fallback_url,
	version: () => undefined,
	is_connected: () => false,
	path_on_url: (p) => get_path_on_api(fallback_url, p),
	set_url: () => undefined
} as ApiContextType);

async function get_version(url: Accessor<URL>) {
	async function version_parser(response: Response): Promise<string> {
		const res = await response.text();
		const prefix = "SegmentedEncoder server";
		if (!res.includes(prefix)) throw new Error("Invalid server", {cause: `response is (${res})`});
		return res.substring(res.indexOf("v", prefix.length));
	}

	//Setup request cancellation
	const controller = new AbortController();
	onCleanup(() => controller.abort());
	const signal = controller.signal

	//Starting to check version
	const path = get_path_on_api(new URL(url()), "/version");
	console.log("Checking version of api at", path.href);
	const request = new Request(path.href, {signal});


	const response = await fetch(request);
	return version_parser(response);
}

/**
 * Receives a signal to the server URL an creates a new signal
 * that gets the server version
 * @param url server url signal
 * @returns Accessor<string | undefined>
 */
function versionWatcher(url: Accessor<URL>): Accessor<string | undefined> {
	const [version, setVersion] = createSignal<string | undefined>(undefined);
	const [repeat, setRepeat] = createSignal(undefined, {equals: false});

	createEffect(() => {
		repeat();
		let timeout = 60000;
		get_version(url).catch((err) => {
			console.warn("Failed to get version:", err);
			//Smaller timeout on error
			timeout = 5000;
		}).then(setVersion).finally(() => setTimeout(setRepeat, timeout));
	});
	createEffect((initial) => {
		if (version() || !initial)
			console.info("Version:", version());
		return false;
	}, true);
	return version;
}

export function ApiProvider(props: ParentProps<{ url?: URL }>) {
	// eslint-disable-next-line solid/reactivity
	const url = router_extract_server_url() ?? props.url ?? fallback_url;
	const [path, setPath] = createSignal(url);
	// eslint-disable-next-line solid/reactivity
	const version = versionWatcher(path);
	const clone_url = () => new URL(path());
	const api: ApiContextType = {
		url: clone_url,
		version,
		is_connected: () => version() != undefined,
		path_on_url: (path) => get_path_on_api(clone_url(), path),
		set_url: setPath
	};
	return (
		<ApiContext.Provider value={api}>
			{props.children}
		</ApiContext.Provider>
	)
}
