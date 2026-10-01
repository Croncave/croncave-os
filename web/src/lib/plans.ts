// Plans in plain words, always from the catalog's numbers (nothing here is a price).

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export type CatalogPlan = Record<string, any>;

export const cap = (s: string) => (s ? s[0].toUpperCase() + s.slice(1) : s);

/** "Every hour", "Every 5 minutes", "Every minute". */
export function cadence(secs: number): string {
	if (secs >= 3600) return secs === 3600 ? 'Every hour' : `Every ${secs / 3600} hours`;
	if (secs >= 120) return `Every ${secs / 60} minutes`;
	return 'Every minute';
}

export function computersLine(p: CatalogPlan): string {
	if (p.computers === 1) return `1 computer, ${cap(p.largest_size)} size`;
	return `${p.computers} computers, up to ${cap(p.largest_size)}`;
}

/** The checklist on a plan card. */
export function features(p: CatalogPlan): string[] {
	const out = [computersLine(p)];
	out.push(p.computers === 1 ? `Checks and scripts ${cadence(p.min_schedule_secs).toLowerCase()}` : cadence(p.min_schedule_secs));
	if (p.awake_at_once > 1 && p.awake_at_once < p.computers && p.keep_awake > 1) out.push(`${p.awake_at_once} awake at the same time`);
	if (p.keep_awake) out.push(`${p.keep_awake} always-awake computer${p.keep_awake === 1 ? '' : 's'}`);
	if (!(p.keep_awake > 1)) out.push(`History kept ${p.history_days} days`);
	if (p.support && p.support !== 'Help docs' && (p.keep_awake === 0 || p.keep_awake > 1)) out.push(`${p.support.replace('Priority email', 'Priority')} support`);
	return out;
}

/** The rows a trial offer compares: what you have, what the trial gives. */
export function compare(from: CatalogPlan, to: CatalogPlan, trialAllowance: string, fromAward: string) {
	const comp = (p: CatalogPlan) => (p.computers === 1 ? `1, ${cap(p.largest_size)}` : `${p.computers}, up to ${cap(p.largest_size)}`);
	return [
		{ label: 'Computers', from: comp(from), to: comp(to) },
		{ label: 'Checks and scripts', from: cadence(from.min_schedule_secs), to: cadence(to.min_schedule_secs) },
		{ label: 'Free usage', from: `${fromAward} to start`, to: `+${trialAllowance} for the trial` },
		{ label: 'History kept', from: `${from.history_days} days`, to: `${to.history_days} days` }
	];
}
