import "./style.css"
import StatusTab from "./components/tabs/status";
import JobsTab from "./components/tabs/jobs";
import ServerSelector from "./components/tabs/serverSelector";
import { Tab, TabBar } from "./components/tabs";
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
				<Tab title={"Server"}>
					<ServerSelector/>
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