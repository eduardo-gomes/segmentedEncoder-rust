import { ApiContext } from "../../lib/api";
import { createSignal, useContext } from "solid-js";
import { textChange } from "../../lib/utils";

function ServerSelector() {
	const {url, set_url} = useContext(ApiContext);
	const [value, setValue] = createSignal("")

	return (
		<>
			<label>Selected server <output>{url().toString()}</output></label>
			<br/>
			Enter new server address to change server.
			<br/>
			<form onSubmit={(e) => {
				e.preventDefault();
				try {
					const url = new URL(value());
					set_url(url);
					console.log("Changed url to", url);
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