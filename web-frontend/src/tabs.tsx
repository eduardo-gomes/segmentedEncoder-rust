import type {Accessor, JSX, ParentComponent} from 'solid-js';
import {children, createEffect, createSelector, createSignal, For, mapArray, Show} from "solid-js";
import type {ResolvedJSXElement} from "solid-js/types/reactive/signal";

type TabComponent = ParentComponent<{ title: string, onVisibilityChange?: (visible: boolean) => void }>;
const Tab: TabComponent = function (props) {
	const onVisibilityChange = (e: CustomEvent<{ visibility: boolean }>) =>
		props.onVisibilityChange?.(e.detail.visibility);

	return (
		<div data-title={props.title} on:VisibilityChange={onVisibilityChange}>
			{props.children}
		</div>
	);
};

function TabBar(props: { children: JSX.Element[] }) {
	const c = children(() => props.children) as Accessor<ResolvedJSXElement[]>;
	const [tabs, setTabs] = createSignal<string[]>([]);
	createEffect(() => {
		const tabs = c();
		const names: string[] = tabs.map(item => (item as HTMLElement).dataset.title ?? "unnamed");
		setTabs(names);
	});
	const [selected, setSelected] = createSignal(0);
	const isSelected = createSelector(selected);

	//Track visibility and emit events to Tab
	mapArray(c, (el, index) => {
		createEffect(() => {
			const visibility = isSelected(index());
			const event = new CustomEvent("VisibilityChange", {detail: {visibility}});
			if (el instanceof Element)
				el.dispatchEvent(event);
		});
	})();

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