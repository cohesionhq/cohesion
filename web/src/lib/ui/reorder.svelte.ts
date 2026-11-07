/**
 * A list the user reorders by dragging, or with Alt+↑/↓ on an item. While
 * an item is dragged, `items` shows where it would land; letting go hands
 * the new order to `onreorder`, if it changed.
 */
export class Reorderable<T> {
	/** The id of the item being dragged. */
	dragged = $state<number | null>(null);
	#preview = $state<T[] | null>(null);
	#items: () => T[];
	#id: (item: T) => number;
	#onreorder: (ids: number[]) => void;

	constructor(items: () => T[], id: (item: T) => number, onreorder: (ids: number[]) => void) {
		this.#items = items;
		this.#id = id;
		this.#onreorder = onreorder;
	}

	/** The items, in the order a drag would leave them. */
	get items(): T[] {
		return this.#preview ?? this.#items();
	}

	#moved(list: T[], id: number, to: number): T[] {
		const from = list.findIndex((item) => this.#id(item) === id);
		const next = list.toSpliced(from, 1);
		next.splice(to, 0, list[from]);
		return next;
	}

	#commit(list: T[]) {
		const items = this.#items();
		if (list.some((item, i) => this.#id(item) !== this.#id(items[i])))
			this.#onreorder(list.map((item) => this.#id(item)));
	}

	/** Starts dragging `item`; `label` is the text a drop elsewhere gets. */
	start(event: DragEvent, item: T, label: string) {
		this.dragged = this.#id(item);
		this.#preview = this.#items();
		event.dataTransfer!.effectAllowed = 'move';
		// Firefox only starts a drag that carries data.
		event.dataTransfer!.setData('text/plain', label);
	}

	/**
	 * The handlers of the list's container, so the whole list, gaps
	 * included, takes the drop.
	 */
	get list() {
		return {
			ondragover: (event: DragEvent) => this.dragged !== null && event.preventDefault(),
			ondrop: (event: DragEvent) => event.preventDefault()
		};
	}

	/**
	 * The handlers of the item at `index`. The dragged item takes its place
	 * once the pointer is past its middle, so it doesn't flip back and forth
	 * at an edge.
	 */
	target(index: number) {
		return {
			ondragover: (event: DragEvent) => {
				if (this.dragged === null || !this.#preview) return;
				const from = this.#preview.findIndex((item) => this.#id(item) === this.dragged);
				if (from === index) return;
				const item = event.currentTarget as HTMLElement;
				const { left, top, width, height } = item.getBoundingClientRect();
				// The sidebar lays its lists out in a row on narrow screens. A list
				// that isn't a flex box still computes `flex-direction: row`.
				const list = getComputedStyle(item.parentElement!);
				const across = list.display.endsWith('flex') && list.flexDirection.startsWith('row');
				const past = across ? event.clientX > left + width / 2 : event.clientY > top + height / 2;
				if (past === from < index) this.#preview = this.#moved(this.#preview, this.dragged, index);
			}
		};
	}

	end() {
		if (this.#preview) this.#commit(this.#preview);
		this.dragged = null;
		this.#preview = null;
	}

	/** Moves the item at `index` up or down on Alt+↑/↓. */
	key(event: KeyboardEvent, index: number) {
		if (!event.altKey || (event.key !== 'ArrowUp' && event.key !== 'ArrowDown')) return;
		event.preventDefault();
		const items = this.#items();
		const to = index + (event.key === 'ArrowUp' ? -1 : 1);
		if (to < 0 || to >= items.length) return;
		this.#commit(this.#moved(items, this.#id(items[index]), to));
	}
}
