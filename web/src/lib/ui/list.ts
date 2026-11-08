// Class lists for a hairline list of rows: a `<ul class={list.rows}>` of
// `<li class={list.row}>`, so every settings section and list page draws the
// same one.

export const list = {
	/** Ruled only between rows, as the Tasks and Documents lists are. */
	rows: '',
	/** One row: a main part and, pushed right, its actions. */
	row: 'flex min-h-11 flex-wrap items-center gap-x-3 gap-y-1 border-border py-2 not-first:border-t',
	/** A section of a list: its label and count over a hairline (the Dashboard's, Documents' days). */
	heading:
		'mb-1 flex items-baseline gap-2 border-b border-border pb-2 text-xs font-semibold text-fg-muted',
	/** The count in a `heading`. */
	count: 'font-normal text-fg-subtle',
	main: 'flex min-w-0 flex-1 flex-wrap items-baseline gap-x-2 gap-y-0 wrap-anywhere',
	actions: 'ml-auto flex flex-none items-center gap-1',
	/** What a list shows when it has nothing in it. */
	empty: 'py-3 text-fg-muted',
	/** Quiet small print under a list. */
	note: 'mt-2 text-xs text-fg-subtle',
	error: 'mt-2 text-xs text-danger'
};
