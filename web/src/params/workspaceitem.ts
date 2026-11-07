import type { ParamMatcher } from '@sveltejs/kit';
import type { WorkspaceItemKind } from '$lib/shell/routes';

/** The kinds of item a `cohesion://` link opens in the workspace it names. */
export const match = ((param: string): param is WorkspaceItemKind =>
	param === 'task' || param === 'document') satisfies ParamMatcher;
