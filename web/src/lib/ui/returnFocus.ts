/**
 * Props for a bits-ui menu or popover `Content` that hand focus back to its
 * trigger on close only when the keyboard closed it (Escape, or Enter on an
 * item). Closed with the pointer, focus is left to fall to the page:
 * returned to the trigger, it would keep a row's `focus-within` tools showing
 * after the pointer had left, as if still hovered.
 *
 * With `returnTo`, focus goes there instead whenever the close came from
 * inside (a key, or a click on an item); a click outside still leaves it
 * where the click put it.
 *
 * Focus is never returned while something on the page has claimed it with
 * `data-claims-focus` (a panel an item opened and focused), so the trigger
 * doesn't take it back whichever of the two runs last.
 */
export function returnFocusFromKeyboard(returnTo?: HTMLElement) {
	let viaKeyboard = false;
	let outside = false;
	return {
		onkeydown: () => {
			viaKeyboard = true;
			outside = false;
		},
		onpointerdown: () => {
			viaKeyboard = false;
			outside = false;
		},
		onInteractOutside: () => {
			viaKeyboard = false;
			outside = true;
		},
		onCloseAutoFocus: (event: Event) => {
			if (document.querySelector('[data-claims-focus]')) event.preventDefault();
			else if (returnTo && !outside) {
				event.preventDefault();
				returnTo.focus();
			} else if (!viaKeyboard) event.preventDefault();
		}
	};
}
