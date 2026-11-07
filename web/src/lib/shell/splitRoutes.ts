// The routes a split view's second half shows, found as SvelteKit finds them:
// every `+page.svelte` under `src/routes` is a route, a `[param]` segment
// takes any value and `[param=matcher]` what `src/params/<matcher>.ts`
// accepts, a `(group)` segment takes none, and a static segment beats a
// matched one, which beats a plain one. The root layout is the shell, so a
// half renders only the layouts below it.

import type { Component, Snippet } from 'svelte';
import { isRedirect } from '@sveltejs/kit';

type Load = (event: { params: Record<string, string>; url: URL; route: { id: string } }) => unknown;

/** A page or a layout, given what its load returned. */
export type RouteComponent = Component<{ data: unknown; children?: Snippet }>;

const pages = import.meta.glob<{ default: RouteComponent }>('/src/routes/**/+page.svelte');
const layouts = import.meta.glob<{ default: RouteComponent }>('/src/routes/*/**/+layout.svelte');
const loads = import.meta.glob<{ load?: Load }>('/src/routes/**/+page.ts');
const matchers = import.meta.glob<{ match: (param: string) => boolean }>('/src/params/*.ts', {
	eager: true
});

/** A route's id, as SvelteKit writes it (`/w/[ws]/tasks`), from its file's path. */
function routeId(file: string, name: string): string {
	return file.slice('/src/routes'.length, -name.length - 1) || '/';
}

const ROUTES = Object.keys(pages).map((file) => routeId(file, '+page.svelte'));

/** What a half shows: a route's page under its layouts, with what its load returned. */
export interface Shown {
	url: URL;
	route: { id: string | null };
	params: Record<string, string>;
	/** Outermost first; empty when no route matches. */
	layouts: RouteComponent[];
	/** Null when no route matches the address. */
	page: RouteComponent | null;
	data: unknown;
}

/** The route `pathname` names and its params, or null when none does. */
export function matchRoute(
	pathname: string
): { id: string; params: Record<string, string> } | null {
	const parts = pathname.split('/').filter(Boolean).map(decodeURIComponent);
	let best: { id: string; params: Record<string, string>; rank: number[] } | null = null;
	for (const id of ROUTES) {
		const segments = id.split('/').filter((s) => s !== '' && !s.startsWith('('));
		if (segments.length !== parts.length) continue;
		const params: Record<string, string> = {};
		const rank: number[] = [];
		const fits = segments.every((segment, i) => {
			const param = /^\[(\w+)(?:=(\w+))?\]$/.exec(segment);
			if (!param) {
				rank.push(2);
				return segment === parts[i];
			}
			const [, name, matcher] = param;
			if (matcher && !matchers[`/src/params/${matcher}.ts`]?.match(parts[i])) return false;
			params[name] = parts[i];
			rank.push(matcher ? 1 : 0);
			return true;
		});
		if (fits && (!best || outranks(rank, best.rank))) best = { id, params, rank };
	}
	return best && { id: best.id, params: best.params };
}

function outranks(a: number[], b: number[]): boolean {
	const i = a.findIndex((n, j) => n !== b[j]);
	return i !== -1 && a[i] > b[i];
}

/** The layouts a route sits under, below the root's, outermost first. */
function layoutFiles(id: string): string[] {
	const segments = id.split('/').filter(Boolean);
	return segments
		.map((_, i) => `/src/routes/${segments.slice(0, i + 1).join('/')}/+layout.svelte`)
		.filter((file) => file in layouts);
}

/** Loads what `href` shows, following its load's redirects. */
export async function resolveRoute(href: string, redirects = 0): Promise<Shown> {
	const url = new URL(href, location.origin);
	const found = matchRoute(url.pathname);
	if (!found) return { url, route: { id: null }, params: {}, layouts: [], page: null, data: null };
	const { id, params } = found;
	const prefix = id === '/' ? '/src/routes' : `/src/routes${id}`;
	const [page, layoutModules, loadModule] = await Promise.all([
		pages[`${prefix}/+page.svelte`](),
		Promise.all(layoutFiles(id).map((file) => layouts[file]())),
		loads[`${prefix}/+page.ts`]?.()
	]);
	let data: unknown;
	try {
		data = (await loadModule?.load?.({ params, url, route: { id } })) ?? null;
	} catch (error) {
		if (isRedirect(error) && redirects < 5) return resolveRoute(error.location, redirects + 1);
		throw error;
	}
	return {
		url,
		route: { id },
		params,
		layouts: layoutModules.map((m) => m.default),
		page: page.default,
		data
	};
}
