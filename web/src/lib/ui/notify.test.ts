import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { notify, openNotificationsWith } from './notify';
import { toasts } from './toasts.svelte';

describe('notify', () => {
	beforeEach(() => {
		vi.useFakeTimers();
		vi.stubGlobal('window', {});
	});
	afterEach(() => {
		vi.runAllTimers();
		vi.useRealTimers();
		vi.unstubAllGlobals();
	});

	it('is a toast where nothing hosts the page', () => {
		notify({ message: 'Ada sent you a message.', href: '/w/1/agents/3' });
		expect(toasts.items).toMatchObject([
			{ message: 'Ada sent you a message.', tone: 'info', href: '/w/1/agents/3' }
		]);
	});

	it('goes to a harness that presents notifications, and isn’t also a toast', () => {
		const present = vi.fn();
		window.cohesionNotify = present;
		notify({ message: 'Backup finished.', title: 'Backups' });
		expect(present).toHaveBeenCalledWith({ message: 'Backup finished.', title: 'Backups' });
		expect(toasts.items).toEqual([]);
	});

	it('is a toast when the harness fails to present it', () => {
		window.cohesionNotify = () => {
			throw new Error('not allowed');
		};
		notify({ message: 'Backup finished.' });
		expect(toasts.items.map((t) => t.message)).toEqual(['Backup finished.']);
	});

	it('opens only the page’s own paths for the harness, until undone', () => {
		const open = vi.fn();
		const undo = openNotificationsWith(open);
		window.cohesionOpen?.('/w/1/agents/3');
		window.cohesionOpen?.('https://example.com/');
		window.cohesionOpen?.('//example.com/');
		expect(open.mock.calls).toEqual([['/w/1/agents/3']]);
		undo();
		expect(window.cohesionOpen).toBeUndefined();
	});
});
