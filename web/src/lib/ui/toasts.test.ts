import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ProtocolError } from '$lib/protocol/errors';
import { UNREACHABLE_MESSAGE, toasts } from './toasts.svelte';

describe('toasts', () => {
	beforeEach(() => vi.useFakeTimers());
	afterEach(() => {
		vi.runAllTimers();
		vi.useRealTimers();
	});

	it('shows one "daemon unreachable" toast however many requests fail', () => {
		toasts.show(UNREACHABLE_MESSAGE, 'error');
		toasts.error(new ProtocolError('disconnected', 'Lost the connection to the daemon.'));
		toasts.error(new ProtocolError('disconnected', 'The daemon is unreachable. Try again.'));
		expect(toasts.items.map((t) => t.message)).toEqual([UNREACHABLE_MESSAGE]);

		// Once it has gone, the next failure shows it again.
		vi.runAllTimers();
		expect(toasts.items).toEqual([]);
		toasts.error(new ProtocolError('disconnected', 'Lost the connection to the daemon.'));
		expect(toasts.items).toHaveLength(1);
	});
});
