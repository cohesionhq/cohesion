// What a row and a list need from a kind's tags: the ones on an item, and the
// "Tags" submenu that puts them on and takes them off.

import type { Tag } from '$lib/protocol/types';
import type { MenuSubmenu } from '$lib/ui/Menu.svelte';

/** The tags of `tags` whose ids `ids` holds, in the tags' own order. */
export function tagsOn(tags: Tag[], ids: number[]): Tag[] {
	return tags.filter((t) => ids.includes(t.id));
}

/** A row's "Tags" submenu: each tag, checked when on the item, then "Manage tags…". */
export function tagsSubmenu(
	tags: Tag[],
	ids: number[],
	ontag: (tag_id: number, tagged: boolean) => void,
	onmanage: () => void
): MenuSubmenu {
	return {
		label: 'Tags',
		items: [
			...tags.map((t) => ({
				id: t.id,
				label: t.name,
				checked: ids.includes(t.id),
				onSelect: () => ontag(t.id, !ids.includes(t.id))
			})),
			{ id: 'manage', label: 'Manage tags…', separated: tags.length > 0, onSelect: onmanage }
		]
	};
}
