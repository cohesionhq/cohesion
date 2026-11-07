import BookOpen from '@lucide/svelte/icons/book-open';
import Terminal from '@lucide/svelte/icons/terminal';
import Wrench from '@lucide/svelte/icons/wrench';
import { describe, expect, it } from 'vitest';
import type { ActorSessionMessageContent } from '$lib/protocol/types';
import { toolIcon, stillRunning } from './tools';

describe('toolIcon', () => {
	it('draws a tool by its category', () => {
		expect(toolIcon('command')).toBe(Terminal);
		expect(toolIcon('other')).toBe(Wrench);
		expect(toolIcon('skill')).toBe(BookOpen);
	});

	it('draws a tool without a category as other', () => {
		expect(toolIcon(undefined)).toBe(Wrench);
	});
});

describe('stillRunning', () => {
	const tool = (ended_at: number | null): ActorSessionMessageContent => ({
		sender: 'actor',
		kind: 'toolUse',
		tool: 'Bash',
		summary: 'ls',
		category: 'command',
		ended_at
	});

	it('runs while the session works and the tool has not ended', () => {
		expect(stillRunning('working', tool(null))).toBe(true);
	});

	it('stops once its actor says it ended', () => {
		expect(stillRunning('working', tool(10))).toBe(false);
	});

	it('stops once the session stops working, even never ended', () => {
		expect(stillRunning('idle', tool(null))).toBe(false);
		expect(stillRunning('failed', tool(null))).toBe(false);
		expect(stillRunning('needsAttention', tool(null))).toBe(false);
	});

	it('runs for thinking the same way', () => {
		const thinking = (ended_at: number | null): ActorSessionMessageContent => ({
			sender: 'actor',
			kind: 'thinking',
			text: 'Maybe the linker.',
			ended_at
		});
		expect(stillRunning('working', thinking(null))).toBe(true);
		expect(stillRunning('working', thinking(10))).toBe(false);
		expect(stillRunning('idle', thinking(null))).toBe(false);
	});

	it('is never a message that does not run', () => {
		expect(stillRunning('working', { sender: 'actor', kind: 'text', text: 'hi' })).toBe(false);
	});
});
