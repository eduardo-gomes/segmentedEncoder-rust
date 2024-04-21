import type { JSX } from "solid-js";
import { createEffect, createMemo, For, onCleanup, untrack } from "solid-js";
import { Tabs } from "@kobalte/core";
import { createSignalObj } from "../lib/utils";

import "../styles/tabs.css";

type TabElement = {
	title: string,
	component: JSX.Element,
	visibilityChange?: (visible: boolean) => void
};

type NonEmptyArray<T> = [T, ...T[]];

function TabBar(props: { children: NonEmptyArray<TabElement> }) {
	const c = createMemo(() => props.children);
	const map = createMemo(() => new Map(c().map((el) => [el.title, el])));
	const active_tab = createSignalObj(untrack((c))[0].title);
	createEffect(() => {
		const active = active_tab.get();
		const callback = map().get(active)?.visibilityChange;
		if (callback) {
			callback(true);
			onCleanup(() => callback(false));
		}
	});

	return (
		<Tabs.Root value={active_tab.get()} onChange={active_tab.set}>
			<Tabs.List class="tabs">
				<For each={c()}>{(tab) =>
					<Tabs.Trigger value={tab.title}>{tab.title}</Tabs.Trigger>
				}</For>
				<Tabs.Indicator class="indicator"/>
			</Tabs.List>
			<For each={c()}>{(tab) =>
				<Tabs.Content value={tab.title}>{tab.component}</Tabs.Content>
			}</For>
		</Tabs.Root>
	)
}

export { TabBar };