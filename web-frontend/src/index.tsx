import "./style.css"
import StatusTab from "./status";
import jobs_div from "./jobs";
import {Tab, TabBar} from "./tabs";
import {render} from "solid-js/web";
import {createSignal} from "solid-js";

function App() {
	const [statusActive, setStatusActive] = createSignal(false);
	return (
		<>
			<TabBar>
				<Tab title={"Status"} onVisibilityChange={setStatusActive}>
					<StatusTab visible={statusActive()}/>
				</Tab>
				<Tab title={"Jobs"}>
					{jobs_div.element}
				</Tab>
			</TabBar>
		</>
	)
}

render(() => <App/>, document.getElementById("App") as HTMLDivElement);