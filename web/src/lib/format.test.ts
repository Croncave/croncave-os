import { describe, expect, it } from 'vitest';
import { ago, bytes, duration, money, statusOf } from './format';

describe('plain formatting', () => {
	it('shows money in dollars, with more digits for tiny amounts', () => {
		expect(money(4_390_000)).toBe('$4.39');
		expect(money(1_234)).toBe('$0.0012');
		expect(money(0)).toBe('$0.00');
	});

	it('shows sizes and durations plainly', () => {
		expect(bytes(512)).toBe('512 B');
		expect(bytes(1_500_000)).toBe('1.4 MB');
		expect(duration(42)).toBe('42s');
		expect(duration(3725)).toBe('1h 2m');
	});

	it('says how long ago', () => {
		const now = Date.parse('2026-10-01T12:00:00Z');
		expect(ago('2026-10-01T11:58:00Z', now)).toBe('2 min ago');
		expect(ago('2026-10-01T12:30:00Z', now)).toBe('in 30 min');
	});

	it('pairs every status with a word', () => {
		expect(statusOf('needs_you')).toEqual({ word: 'Needs you', tone: 'needs' });
		expect(statusOf('asleep').word).toBe('Asleep');
		expect(statusOf('timed_out').tone).toBe('failed');
	});
});
