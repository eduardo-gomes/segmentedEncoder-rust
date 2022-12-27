import type {JSX, ParentComponent} from 'solid-js';
import {children, createEffect, createSelector, createSignal, For, Show} from "solid-js";
import type {ResolvedJSXElement} from "solid-js/types/reactive/signal";

type TabComponent = ParentComponent<{ title: string }>;
const Tab: TabComponent = function (props) {
	return <div data-title={props.title}>{props.children}</div>
};

function TabBar(props: { children: JSX.Element[] }) {
	const c = children(() => props.children);
	const [tabs, setTabs] = createSignal<string[]>([]);
	createEffect(() => {
		const tabs = c() as ResolvedJSXElement[];
		const names: string[] = tabs.map(item => (item as HTMLElement).dataset.title ?? "unnamed");
		setTabs(names);
	});
	const [selected, setSelected] = createSignal(0);
	const isSelected = createSelector(selected);

	return (
		<>
			<div class={"tabs"}>
				<For each={tabs()}>
					{(tab, index) =>
						<button onClick={() => setSelected(index())}>{tab}</button>}
				</For>
			</div>
			<For each={c() as ResolvedJSXElement[]}>
				{(tab, index) =>
					<Show when={isSelected(index())}>
						{tab}
					</Show>}
			</For>
		</>
	)
}

export {TabBar, Tab};