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

// ---- The folder switcher under a page's heading (`FolderHeading`) ---------------------------

/** A row of the switcher: a link, then its tools at the end. The row of what's shown is filled. */
export const switcherRow =
	'group/row relative flex min-h-7.5 items-center gap-1 rounded-sm hover:bg-bg-hover has-[[aria-current=page]]:bg-current-item';

/** A row's link, holding its icon and title; `aria-current="page"` marks what's shown. */
export const switcherLink =
	'flex min-w-0 flex-1 items-center gap-2 self-stretch rounded-sm px-2 text-fg-muted no-underline hover:text-fg hover:no-underline aria-[current=page]:font-medium aria-[current=page]:text-fg [&_svg]:flex-none [&_svg]:text-fg-subtle';

/** A row's tools (pin, rename, delete): shown on hover or focus, and always on touch. */
export const switcherTools =
	'flex flex-none items-center pr-1 opacity-0 transition-opacity group-focus-within/row:opacity-100 group-hover/row:opacity-100 touch:opacity-100';

/** One of a row's tools, an icon button. */
export const switcherTool =
	'inline-flex size-6 cursor-pointer items-center justify-center rounded-sm text-fg-subtle hover:text-fg';

/** A group's label in the switcher, such as "Views". */
export const switcherHeading = 'px-2 pt-2 pb-1 text-xs font-medium text-fg-subtle';

/** A rule between the switcher's groups. */
export const switcherRule = 'mx-1 my-1 h-px flex-none bg-border';

/** A quiet button ending the switcher, such as "New folder…". */
export const switcherAction =
	'flex min-h-7.5 cursor-pointer items-center gap-2 rounded-sm px-2 text-left text-fg-subtle hover:bg-bg-hover hover:text-fg';
