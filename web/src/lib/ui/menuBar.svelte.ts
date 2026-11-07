// What a MenuBar shares with its items: which item's panel is showing,
// and which way to slide when the panel moves to another item.

import { createContext, type Snippet } from 'svelte';

/** One item of a menu bar: its chip, and the panel it opens onto. */
export interface MenuBarEntry {
	id: string;
	anchor: HTMLElement;
	/** The panel's contents, handed a function that closes it. */
	panel: Snippet<[close: () => void]>;
}

export class MenuBarState {
	open = $state(false);
	/** The item the panel belongs to, kept after closing so it fades out where it was. */
	active = $state<MenuBarEntry | null>(null);
	/** Whether the panel has moved since it opened, which is when it slides. */
	moved = $state(false);
	/** Which way the new contents slide in: 1 from the right, -1 from the left. */
	direction = $state(0);
	/** The item the pointer just moved the panel to, whose click would otherwise close it again. */
	private pointedAt: string | null = null;

	/** Opens `entry`'s panel, or closes it if it is the one showing. */
	toggle(entry: MenuBarEntry) {
		if (this.open && this.pointedAt === entry.id) this.pointedAt = null;
		else if (this.open && this.active?.id === entry.id) this.close();
		else this.show(entry);
	}

	/** Moves an open panel to `entry`, as the pointer crosses onto it. */
	hover(entry: MenuBarEntry) {
		if (!this.open || this.active?.id === entry.id) return;
		this.show(entry);
		this.pointedAt = entry.id;
	}

	close = () => {
		this.open = false;
	};

	/** Closes the panel if it belongs to `id`, which is going away. */
	forget(id: string) {
		if (this.active?.id === id) this.close();
	}

	private show(entry: MenuBarEntry) {
		if (this.open && this.active && this.active.id !== entry.id) {
			const after =
				this.active.anchor.compareDocumentPosition(entry.anchor) & Node.DOCUMENT_POSITION_FOLLOWING;
			this.direction = after ? 1 : -1;
			this.moved = true;
		} else if (!this.open) {
			this.direction = 0;
			this.moved = false;
		}
		this.active = entry;
		this.open = true;
		this.pointedAt = null;
	}
}

export const [getMenuBar, setMenuBar] = createContext<MenuBarState>();
