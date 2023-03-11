import { Accessor, createContext, createEffect, createSignal, onCleanup, ParentProps, Setter } from "solid-js";

function get_path_on_api(url: URL, path: string) {
	url.pathname += path;
	console.debug(url);
	return url;
}

export type ApiContextType = {
	url: Accessor<URL>,
	/**Return server version string if is connected or undefined*/
	version: Accessor<string | undefined>,
	set_url: Setter<URL>
	path_on_url: (path: string) => URL
};

const fallback_url = new URL("http://localhost:8888/api");

export const ApiContext = createContext<ApiContextType>({
	url: () => fallback_url,
	version: () => undefined,
	path_on_url: (p) => get_path_on_api(fallback_url, p),
	set_url: () => undefined
} as ApiContextType);

function versionWatcher(url: Accessor<URL>): Accessor<string | undefined> {
	const [version, setVersion] = createSignal<string | undefined>(undefined);

	async function version_parser(response: Response): Promise<string> {
		const res = await response.text();
		const prefix = "SegmentedEncoder server";
		if (!res.includes(prefix)) throw new Error("Invalid server", {cause: `response is (${res})`});
		return res.substring(res.indexOf("v", prefix.length));
	}

	createEffect(() => {
		setVersion(undefined);
		const path = get_path_on_api(new URL(url()), "/version");
		console.log("Checking version of api at", path.href);

		const controller = new AbortController();
		onCleanup(() => controller.abort());
		const signal = controller.signal
		const request = new Request(path.href, {signal});

		function fetch_reject(err: unknown): Promise<Response> {
			console.error("Fetch failed:", err);
			if (err instanceof DOMException && err.name === "AbortError")
				return Promise.reject(err);
			else
				return fetch(request).catch(fetch_reject);
		}

		fetch(request)
			.catch(fetch_reject)
			.then((res) =>
				version_parser(res)
					.then(setVersion)
					.catch((err) =>
						console.error("Is not segmentedEncoder server", err)
					)
			);
	});
	createEffect((initial) => {
		if (version() || !initial)
			console.info("Version:", version());
		return false;
	}, true);
	return version;
}

export function ApiProvider(props: ParentProps<{ url: URL }>) {
	// eslint-disable-next-line solid/reactivity
	const [path, setPath] = createSignal(props.url);
	const version = versionWatcher(path);
	const clone_url = () => new URL(path());
	const api: ApiContextType = {
		url: clone_url,
		version,
		path_on_url: (path) => get_path_on_api(clone_url(), path),
		set_url: setPath
	};
	return (
		<ApiContext.Provider value={api}>
			{props.children}
		</ApiContext.Provider>
	)
}