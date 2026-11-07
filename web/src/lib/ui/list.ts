// Class lists for a hairline list of rows: a `<ul class={list.rows}>` of
// `<li class={list.row}>`, so every settings section and list page draws the
// same one.

export const list = {
	rows: 'border-t border-border',
	/** One row: a main part and, pushed right, its actions. */
	row: 'flex min-h-11 flex-wrap items-center gap-x-3 gap-y-1 border-b border-border py-2',
	main: 'flex min-w-0 flex-1 flex-wrap items-baseline gap-x-2 gap-y-0 wrap-anywhere',
	actions: 'ml-auto flex flex-none items-center gap-1',
	/** What a list shows when it has nothing in it. */
	empty: 'border-b border-border py-3 text-fg-muted',
	/** Quiet small print under a list. */
	note: 'mt-2 text-xs text-fg-subtle',
	error: 'mt-2 text-xs text-danger'
};
