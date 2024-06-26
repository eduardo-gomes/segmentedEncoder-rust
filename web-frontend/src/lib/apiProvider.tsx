import type { Accessor, ParentProps, Setter } from "solid-js";
import { createContext, createEffect, createSignal, onCleanup, untrack } from "solid-js";
import { router_extract_server_url } from "./router_util";
import { BASE_PATH, Configuration, DefaultApi } from "@api";
import { createSignalObj, type Signal } from "./utils";

export type ApiContextType = {
	api: Accessor<DefaultApi>
	version: Accessor<string | undefined>
	path: Signal<URL>
	authenticated: Accessor<boolean>
	set_password: Setter<string>
};

const fallback_url = new URL(BASE_PATH);

export const ApiContext = createContext<ApiContextType>({
	api: () => new DefaultApi(),
	version: () => undefined,
	path: { get: () => fallback_url, set: () => undefined },
	authenticated: () => false,
	set_password: () => undefined
} as ApiContextType);


function versionWatcher(api: DefaultApi): Accessor<string | undefined> {
	const [version, setVersion] = createSignal<string | undefined>(undefined);
	const [sinceOk, setSinceOk] = createSignal(0, { equals: false });
	const LONG_UPDATE = 30;
	createEffect(() => {
		if (sinceOk() % LONG_UPDATE) return;
		api.versionGet().catch((err) => {
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
	const path = createSignalObj(url);
	createEffect(() => {
		const url = router_extract_server_url();
		if (url)
			path.set(url);
	}, undefined, { name: "provider_extract_url" });
	const [gen, setGen] = createSignal(new DefaultApi());
	const [watcher, setWatcher] = createSignal<Accessor<string | undefined>>();
	const [version, setVersion] = createSignal(undefined as undefined | string);
	const [key, setKey] = createSignal<string | undefined>();
	const password = createSignalObj("password");
	createEffect(() => {
		setKey(undefined);
		const api = new DefaultApi(new Configuration({ basePath: path.get().href }));
		setWatcher(() => versionWatcher(api));
	}, undefined, { name: "provider_update_api" });
	createEffect(() => {
		setGen(new DefaultApi(new Configuration({ basePath: path.get().href, apiKey: key() })));
	});
	createEffect(() => {
		const got_watcher = watcher();
		const version = got_watcher ? got_watcher() : undefined;
		setVersion(version);
	}, undefined, { name: "version_watcher" });
	createEffect(() => {
		const isConnectedToNewServer = Boolean(version());
		const credentials = password.get();
		if (isConnectedToNewServer) {
			const abort = new AbortController();
			untrack(gen).loginGet({ credentials }).then(setKey);
			onCleanup(() => abort.abort("Refreshed"));
		}
	});
	const api: ApiContextType = {
		api: gen,
		version,
		path,
		authenticated: () => Boolean(key()) && Boolean(version()),
		set_password: password.set
	};
	return (
		<ApiContext.Provider value={api}>
			{props.children}
		</ApiContext.Provider>
	)
}
