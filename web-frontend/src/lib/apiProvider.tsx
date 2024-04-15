import type {Accessor, ParentProps, Setter,} from "solid-js";
import {createContext, createEffect, createSignal} from "solid-js";
import {router_extract_server_url} from "./router_util";
import { Configuration, DefaultApi } from "@api";

export type ApiContextType = {
	api: Accessor<DefaultApi>
	version: Accessor<string | undefined>
	path: Accessor<URL>
	set_path: Setter<URL>
};

const fallback_url = new URL("http://localhost:8888/api");

export const ApiContext = createContext<ApiContextType>({
	api: () => new DefaultApi(),
	version: () => undefined,
	path: () => fallback_url,
	set_path: () => undefined
} as ApiContextType);


function versionWatcher(api: Accessor<DefaultApi>): Accessor<string | undefined> {
	const [version, setVersion] = createSignal<string | undefined>(undefined);
	const [repeat, setRepeat] = createSignal(undefined, {equals: false});

	createEffect(() => {
		repeat();
		let timeout = 60000;
		api().versionGet().catch((err) => {
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
	const url = props.url ?? fallback_url;
	const [path, setPath] = createSignal(url);
	createEffect(() => {
		const url = router_extract_server_url();
		if(url)
			setPath(url);
	},undefined, {name: "provider_extract_url"});
	const [gen, setGen] = createSignal(new DefaultApi());
	let version: Accessor<string | undefined> = versionWatcher(gen);
	createEffect(() => {
		setGen(new DefaultApi(new Configuration({basePath: path().href})));
		version = versionWatcher(gen)
	},undefined, {name: "provider_update_api"});
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
