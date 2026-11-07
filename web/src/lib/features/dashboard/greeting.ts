/**
 * The Dashboard's greeting: a line for the time of day, picked
 * from a few that range from plain to playful, every one naming the user.
 */

type Line = (name: string) => string;

/** From the hour each part of the day starts, latest first. */
const parts: { from: number; lines: Line[]; plain: string }[] = [
	{
		from: 21,
		plain: 'Good evening',
		lines: [
			(n) => `Good evening, ${n}`,
			(n) => `Still at it, ${n}?`,
			(n) => `Night shift, ${n}?`,
			(n) => `One last thing, ${n}?`
		]
	},
	{
		from: 17,
		plain: 'Good evening',
		lines: [
			(n) => `Good evening, ${n}`,
			(n) => `Evening, ${n}`,
			(n) => `Winding down, ${n}?`,
			(n) => `Home stretch, ${n}`
		]
	},
	{
		from: 12,
		plain: 'Good afternoon',
		lines: [
			(n) => `Good afternoon, ${n}`,
			(n) => `Afternoon, ${n}`,
			(n) => `Back at it, ${n}?`,
			(n) => `Halfway there, ${n}`
		]
	},
	{
		from: 8,
		plain: 'Good morning',
		lines: [
			(n) => `Good morning, ${n}`,
			(n) => `Morning, ${n}`,
			(n) => `Fresh start, ${n}`,
			(n) => `Let’s get going, ${n}`
		]
	},
	{
		from: 5,
		plain: 'Good morning',
		lines: [
			(n) => `Good morning, ${n}`,
			(n) => `Early start, ${n}?`,
			(n) => `Up with the sun, ${n}?`,
			(n) => `Coffee first, ${n}?`
		]
	},
	{
		from: 0,
		plain: 'Hello',
		lines: [
			(n) => `Hello, ${n}`,
			(n) => `Late night, ${n}?`,
			(n) => `Still up, ${n}?`,
			(n) => `Burning the midnight oil, ${n}?`
		]
	}
];

/**
 * The greeting for `hour` (0–23), the line chosen by `pick` (0 to 1, such
 * as `Math.random()`); without a name, the plain greeting for the hour.
 */
export function greeting(hour: number, name: string | null, pick: number): string {
	const part = parts.find((p) => hour >= p.from)!;
	if (!name) return part.plain;
	return part.lines[Math.floor(pick * part.lines.length) % part.lines.length](name);
}
