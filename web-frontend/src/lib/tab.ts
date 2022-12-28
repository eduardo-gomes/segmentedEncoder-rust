class Tab {
	private readonly div: HTMLDivElement;
	private readonly foreground: (() => void) | undefined;
	private readonly background: (() => void) | undefined;
	private readonly _label: string;

	constructor(div: HTMLDivElement, label: string, foreground?: () => void, background?: () => void) {
		this.div = div;
		this._label = label;
		this.foreground = foreground;
		this.background = background;
	}

	show() {
		this.div.classList.remove("disabled");
		if (this.foreground)
			this.foreground();
		console.debug("Show", this._label);
	}

	hide() {
		this.div.classList.add("disabled");
		if (this.background)
			this.background();
	}

	get element() {
		return this.div;
	}

	get label() {
		return this._label;
	}
}

export default Tab;