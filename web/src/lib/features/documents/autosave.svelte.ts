// Wires a Draft to the page lifecycle the way Today does: save when its half
// of the sheet leaves the page, when the tab hides and on unmount, and toast
// a failed save.

import { pane } from '$lib/shell/pane.svelte';
import type { Draft } from '$lib/stores/draft.svelte';
import { toasts } from '$lib/ui/toasts.svelte';

/** Call from a page's script. Returns a `save` for the editor's blur. */
export function useAutosave(draft: Draft): () => void {
	const save = () => {
		draft.flush().catch(() => undefined);
	};

	pane().beforeLeave(save);

	$effect(() => {
		const onHide = () => {
			if (document.visibilityState === 'hidden') save();
		};
		document.addEventListener('visibilitychange', onHide);
		return () => {
			document.removeEventListener('visibilitychange', onHide);
			save();
		};
	});

	$effect(() => {
		if (draft.saveError) toasts.error(draft.saveError);
	});

	return save;
}
