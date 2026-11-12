import { describe, expect, it } from 'vitest';
import { routes, workspaceSwitchRoute } from './routes';

describe('workspaceSwitchRoute', () => {
	it('keeps the kind of page', () => {
		expect(workspaceSwitchRoute('/w/1/documents', 2)).toBe(routes.documents(2));
		expect(workspaceSwitchRoute('/w/1/tasks', 2)).toBe(routes.tasks(2));
		expect(workspaceSwitchRoute('/w/1/articles/research', 2)).toBe(routes.research(2));
	});

	it('opens Tasks from a task folder, and the list from an item page', () => {
		// A task folder is the Tasks page at `?view=`, which the path ignores.
		expect(workspaceSwitchRoute('/w/1/tasks', 2)).toBe(routes.tasks(2));
		expect(workspaceSwitchRoute('/w/1/documents/9', 2)).toBe(routes.documents(2));
		expect(workspaceSwitchRoute('/w/1/journal/4', 2)).toBe(routes.journal(2));
	});

	it('opens the Dashboard from a resource page or outside a workspace', () => {
		expect(workspaceSwitchRoute('/w/1/resources/3', 2)).toBe(routes.dashboard(2));
		expect(workspaceSwitchRoute('/agents', 2)).toBe(routes.dashboard(2));
	});
});
