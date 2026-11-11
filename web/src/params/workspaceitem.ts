import type { ParamMatcher } from '@sveltejs/kit';
import { isWorkspaceItemKind } from '$lib/format/itemLinks';

/** The kinds of item a `cohesion://` link opens in the workspace it names. */
export const match = isWorkspaceItemKind satisfies ParamMatcher;
