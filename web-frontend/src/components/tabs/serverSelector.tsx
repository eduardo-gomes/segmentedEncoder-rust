import { createSignal, useContext } from "solid-js";
import { createSignalObj, textChange } from "../../lib/utils";
import { ApiContext } from "../../lib/apiProvider";

function ServerSelector() {
	const { authenticated, version, path, set_password } = useContext(ApiContext);
	const [value, setValue] = createSignal("")
	const password = createSignalObj("");

	function submit_url(e: Event) {
		e.preventDefault();
		try {
			const url = new URL(value());
			path.set(url);
			console.log("Changed url to", url.href);
		} catch (e) {
			alert(e)
		}
	}

	function submit_password(e: Event) {
		e.preventDefault();
		set_password(password.get());
	}

	return (
		<>
			<label>Selected server <output>{path.get().href}</output></label>
			<br/>
			<label>Version: <output>{version() ?? "Not connected"}</output></label>
			<br/>
			<label>Authenticated: <output>{String(authenticated())}</output></label>
			<br/>
			Enter new server address to change server.
			<br/>
			<form onSubmit={submit_url}>
				<label>New server address:
					<input type="text" value={value()} onChange={textChange(setValue)}/>
				</label>
				<input type="submit" value="Set address"/>
			</form>
			<form onSubmit={submit_password}>
				<label>Change password:
					<input type="password" value={password.get()} onChange={textChange(password.set)}/>
				</label>
				<input type="submit" value="Set password"/>
			</form>
		</>
	)
}

export default ServerSelector;