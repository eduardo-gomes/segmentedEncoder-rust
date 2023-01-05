import { get_path_on_api } from "./lib/api";
import { createEffect, createSignal, onCleanup } from "solid-js";

function StatusTab(props: { visible: boolean }) {
	const [status, setStatus] = createSignal("");

	async function refresh() {
		let res;
		try {
			res = await fetch(get_path_on_api("/status"));
		} catch (e) {
			const message = "Fetch failed";
			setStatus(message);
			throw new Error(message, {cause: e as Error});
		}
		if (res.status >= 400) {
			const message = `Refresh got status code: ${res.status}`;
			setStatus(message);
			throw new Error(message);
		}
		setStatus(await res.text());
		console.debug("Request got:", res.status);
	}

	let timeout: undefined | number;
	let should_rerun = false;

	function rerun() {
		clearTimeout(timeout);
		if (should_rerun)
			timeout = setTimeout(status_updater, 1000);
	}

	function status_updater() {
		refresh().catch((e) => console.error("Failed to update status:", e)).finally(rerun);
	}

	createEffect(() => {
		should_rerun = props.visible;
		if (should_rerun)
			status_updater();
		onCleanup(() => {
			should_rerun = false;
			clearTimeout(timeout);
		});
	});

	return (<>
		Auto refreshing /latest:
		<pre>{status()}</pre>
	</>);
}

export default StatusTab;