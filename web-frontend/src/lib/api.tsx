import { Accessor, createContext, createSignal, ParentProps, Setter } from "solid-js";

function get_path_on_api(url: URL, path: string) {
	url.pathname += path;
	console.log(url);
	return url;
}

export type ApiContextType = {
	url: Accessor<URL>,
	set_url: Setter<URL>
	path_on_url: (path: string) => URL
};

const fallback_url = new URL("http://localhost:8888/api");

export const ApiContext = createContext<ApiContextType>({
	url: () => fallback_url,
	path_on_url: (p) => get_path_on_api(fallback_url, p),
	set_url: () => undefined
} as ApiContextType);

export function ApiProvider(props: ParentProps<{ url: URL }>) {
	// eslint-disable-next-line solid/reactivity
	const [path, setPath] = createSignal(props.url);
	const clone_url = () => new URL(path());
	const api: ApiContextType = {
		url: clone_url,
		path_on_url: (path) => get_path_on_api(clone_url(), path),
		set_url: setPath
	};
	return (
		<ApiContext.Provider value={api}>
			{props.children}
		</ApiContext.Provider>
	)
}