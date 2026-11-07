import { describe, expect, it } from 'vitest';
import { matchRoute } from './splitRoutes';

describe('matchRoute', () => {
	it('finds a route and its params', () => {
		expect(matchRoute('/w/3/tasks')).toEqual({ id: '/w/[ws]/tasks', params: { ws: '3' } });
		expect(matchRoute('/')).toEqual({ id: '/', params: {} });
	});

	it('prefers a static segment to a param', () => {
		expect(matchRoute('/w/3/articles/research')?.id).toBe('/w/[ws]/articles/research');
		expect(matchRoute('/w/3/articles/9')?.id).toBe('/w/[ws]/articles/[id]');
	});

	it('takes a matched param only for what its matcher accepts', () => {
		expect(matchRoute('/open/agent/2')).toEqual({
			id: '/open/[kind=item]/[id]',
			params: { kind: 'agent', id: '2' }
		});
		expect(matchRoute('/open/document/1/4')?.id).toBe('/open/[kind=workspaceitem]/[ws]/[id]');
		expect(matchRoute('/open/agent/1/4')).toBeNull();
	});

	it('skips a group segment', () => {
		expect(matchRoute('/extensions/installed')?.id).toBe('/extensions/(tabs)/installed');
		expect(matchRoute('/extensions/installed/5')?.id).toBe('/extensions/installed/[id]');
	});

	it('finds nothing for an unknown address', () => {
		expect(matchRoute('/nowhere/at/all')).toBeNull();
	});
});
