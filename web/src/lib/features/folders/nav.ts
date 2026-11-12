// The look of a page's folder nav, shared by the links a page adds around
// its folders (an inbox, All, feeds) so they match the folders themselves.

/** A row of the nav; the row of the view shown is filled, as in the app's sidebar. */
export const navItem =
	'flex items-center rounded-sm hover:bg-bg-hover has-[[aria-current=page]]:bg-current-item';

/** A row's link; `aria-current="page"` marks the view shown. */
export const navLink =
	'min-w-0 flex-1 truncate rounded-sm px-2 py-1 text-fg-muted hover:text-fg hover:no-underline aria-[current=page]:font-medium aria-[current=page]:text-fg';

/** A quiet count at a row's end: a feed's unread articles. */
export const navCount = 'flex-none px-2 text-xs text-fg-subtle tabular-nums';

/** A group's label, such as "Feeds", as a row of its list: a step down from the group above it. */
export const navHeading =
	'mt-4 px-2 pb-1 text-xs font-medium text-fg-subtle first:mt-0 max-md:hidden';

/** A quiet button ending a group, such as "New folder…": a row on the folders' rhythm, in the sidebar's "New agent" grey. */
export const navAction =
	'flex cursor-pointer items-center gap-2 rounded-sm px-2 py-1 text-left text-fg-subtle hover:bg-bg-hover hover:text-fg';

/** The pane holding a page's folder nav: under the page's header, pinned there as the list
 * scrolls, and scrolled on its own, so a long nav and a long list don't scroll each other
 * away. Its scrollbar is hidden, as the app sidebar's is, so overflowing never
 * narrows it, and it gives back the main pane's bottom padding, so at full
 * height it never makes the main pane scroll too. Above the list on narrow
 * screens, where it scrolls with it. */
export const navPane =
	'sticky top-(--page-header-height) -mb-12 max-h-[calc(100dvh-var(--titlebar-inset)-var(--page-header-height))] w-(--sidebar-width) flex-none [scrollbar-width:none] self-start overflow-y-auto pb-6 max-md:static max-md:mb-0 max-md:max-h-none max-md:w-auto max-md:overflow-visible max-md:pb-0';
