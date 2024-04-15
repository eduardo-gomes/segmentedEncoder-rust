import { createSignal, useContext } from "solid-js";
import { textChange } from "../../lib/utils";
import { ApiContext } from "../../lib/apiProvider";

function ServerSelector() {
	const {api, version, set_path, path} = useContext(ApiContext);
	const [value, setValue] = createSignal("")

	return (
		<>
			<label>Selected server <output>{path().href}</output></label>
			<br/>
			<label>Version: <output>{version() ?? "Not connected"}</output></label>
			<br/>
			Enter new server address to change server.
			<br/>
			<form onSubmit={(e) => {
				e.preventDefault();
				try {
					const url = new URL(value());
					set_path(url);
					console.log("Changed url to", url.href);
				} catch (e) {
					alert(e)
				}
			}}>
				<label>New server address:
					<input type="text" value={value()} onChange={textChange(setValue)}/>
				</label>
				<input type="submit" value="Set address"/>
			</form>
		</>
	)
}

export default ServerSelector;