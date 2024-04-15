import type { Accessor, ParentProps, Setter } from "solid-js";
import { createContext, createEffect, createSignal, onCleanup, untrack } from "solid-js";
import { router_extract_server_url } from "./router_util";
import { BASE_PATH, Configuration, DefaultApi } from "@api";

export type ApiContextType = {
	api: Accessor<DefaultApi>
	version: Accessor<string | undefined>
	path: Accessor<URL>
	set_path: Setter<URL>
};

const fallback_url = new URL(BASE_PATH);

export const ApiContext = createContext<ApiContextType>({
	api: () => new DefaultApi(),
	version: () => undefined,
	path: () => fallback_url,
	set_path: () => undefined
} as ApiContextType);


function versionWatcher(api: Accessor<DefaultApi>): Accessor<string | undefined> {
	const [version, setVersion] = createSignal<string | undefined>(undefined);
	const [sinceOk, setSinceOk] = createSignal(0, { equals: false });
	const LONG_UPDATE = 30;
	createEffect(() => {
		api();
		setSinceOk(0);
	}, undefined, { name: "Track api changes" });
	createEffect(() => {
		if (sinceOk() % LONG_UPDATE) return;
		api().versionGet().catch((err) => {
			console.warn("Failed to get version:", err);
		}).then(setVersion);
	});

	function increment() {
		if (version())
			setSinceOk((val) => val + 1);
		else
			setSinceOk(0);
	}

	const interval = setInterval(increment, 5000);
	onCleanup(() => clearInterval(interval));
	createEffect((initial) => {
		if (version() || !initial)
			console.info("Version:", version());
		return false;
	}, true);
	return version;
}

export function ApiProvider(props: ParentProps<{ url?: URL }>) {
	const url = untrack(() => props.url) ?? fallback_url;
	const [path, setPath] = createSignal(url);
	createEffect(() => {
		const url = router_extract_server_url();
		if (url)
			setPath(url);
	}, undefined, { name: "provider_extract_url" });
	const [gen, setGen] = createSignal(new DefaultApi());
	// eslint-disable-next-line solid/reactivity
	const version: Accessor<string | undefined> = versionWatcher(gen);
	createEffect(() => {
		setGen(new DefaultApi(new Configuration({ basePath: path().href })));
	}, undefined, { name: "provider_update_api" });
	const api: ApiContextType = {
		api: gen,
		version,
		path,
		set_path: setPath
	};
	return (
		<ApiContext.Provider value={api}>
			{props.children}
		</ApiContext.Provider>
	)
}
