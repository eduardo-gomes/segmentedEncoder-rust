import { createSignal, Match, Switch, useContext } from "solid-js";
import { ApiContext } from "../../lib/apiProvider";

function StatusTab(props: { visible: boolean }) {
	const [status, _setStatus] = createSignal("Not yet supported on new api");

	const {version} = useContext(ApiContext);
	const is_connected = () => Boolean(version);

	return (<>
		Auto refreshing /latest:
		<Switch fallback={<div style={{"font-size": "x-large"}}>Not connected to the server</div>}>
			<Match when={is_connected()}>
				<pre>{status()}</pre>
			</Match>
		</Switch>
	</>);
}

export default StatusTab;