import "./style.css"
import StatusTab from "./status";
import JobsTab from "./jobs";
import { Tab, TabBar } from "./tabs";
import { render } from "solid-js/web";
import { createSignal } from "solid-js";
import { ApiProvider } from "./lib/api";

function App() {
	const [statusActive, setStatusActive] = createSignal(false);
	return (
		<>
			<TabBar>
				<Tab title={"Status"} onVisibilityChange={setStatusActive}>
					<StatusTab visible={statusActive()}/>
				</Tab>
				<Tab title={"Jobs"}>
					<JobsTab/>
				</Tab>
			</TabBar>
		</>
	)
}

render(() => (
	<ApiProvider url={new URL("http://localhost:8888/api")}>
		<App/>
	</ApiProvider>
), document.getElementById("App") as HTMLDivElement);