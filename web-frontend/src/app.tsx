import "./style.css"
import { createSignal } from "solid-js";
import { TabBar } from "./components/tabs";
import StatusTab from "./components/tabs/status";
import JobsTab from "./components/tabs/jobs";
import ServerSelector from "./components/tabs/serverSelector";
import { ApiProvider } from "./lib/apiProvider";

export function App() {
	const [statusActive, setStatusActive] = createSignal(false);
	return (
		<ApiProvider>
			<TabBar>
				{[
					{
						title: "Status",
						component: <StatusTab visible={statusActive()}/>,
						visibilityChange: setStatusActive
					},
					{
						title: "Jobs",
						component: <JobsTab/>,
					},
					{
						title: "Server",
						component: <ServerSelector/>,
					},
				]}
			</TabBar>
		</ApiProvider>
	)
}
