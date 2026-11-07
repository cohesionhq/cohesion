// The quiet <details> disclosures the agent feed and the thread transcript
// share: a small summary over a scrolling code block.

export const summaryClass =
	'w-fit cursor-pointer text-xs text-fg-subtle select-none group-open:mb-1 hover:text-fg-muted';

export const preClass =
	'max-h-90 overflow-y-auto border-l-2 border-border px-3 py-2 font-mono text-xs wrap-anywhere whitespace-pre-wrap';

/** A code block inside a step that opens (`ToolRow`), which already draws the rule beside it. */
export const toolPreClass =
	'max-h-90 overflow-y-auto rounded-sm bg-bg-raised px-3 py-2 font-mono text-xs wrap-anywhere whitespace-pre-wrap';
