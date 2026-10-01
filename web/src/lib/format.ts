export function money(micros: number | null | undefined): string {
	const d = (micros ?? 0) / 1_000_000;
	if (d === 0) return '$0.00';
	if (Math.abs(d) < 0.01) return `$${d.toFixed(4)}`;
	return `$${d.toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 })}`;
}

export function dollars(d: number): string {
	return `$${d.toLocaleString('en-US', { minimumFractionDigits: d % 1 ? 2 : 0, maximumFractionDigits: 2 })}`;
}

export function ago(iso: string | null | undefined, now = Date.now()): string {
	if (!iso) return '';
	const s = Math.round((now - new Date(iso).getTime()) / 1000);
	if (s < -60) return `in ${until(-s)}`;
	if (s < 10) return 'just now';
	if (s < 60) return `${s}s ago`;
	if (s < 3600) return `${Math.floor(s / 60)} min ago`;
	if (s < 86400) return `${Math.floor(s / 3600)}h ago`;
	return new Date(iso).toLocaleDateString(undefined, { month: 'short', day: 'numeric' });
}

function until(s: number): string {
	if (s < 3600) return `${Math.max(1, Math.round(s / 60))} min`;
	if (s < 86400) return `${Math.round(s / 3600)}h`;
	return `${Math.round(s / 86400)} days`;
}

export function when(iso: string | null | undefined): string {
	if (!iso) return '';
	return new Date(iso).toLocaleString(undefined, { month: 'short', day: 'numeric', hour: 'numeric', minute: '2-digit' });
}

export function duration(secs: number): string {
	if (secs < 1) return 'under a second';
	if (secs < 60) return `${Math.round(secs)}s`;
	if (secs < 3600) return `${Math.floor(secs / 60)}m ${Math.round(secs % 60)}s`;
	return `${Math.floor(secs / 3600)}h ${Math.round((secs % 3600) / 60)}m`;
}

export function between(a: string | null, b: string | null): string {
	if (!a) return '';
	const end = b ? new Date(b).getTime() : Date.now();
	return duration((end - new Date(a).getTime()) / 1000);
}

export function bytes(n: number | null | undefined): string {
	const v = n ?? 0;
	if (v < 1024) return `${v} B`;
	const units = ['KB', 'MB', 'GB', 'TB'];
	let x = v / 1024;
	let i = 0;
	while (x >= 1024 && i < units.length - 1) {
		x /= 1024;
		i++;
	}
	return `${x < 10 ? x.toFixed(1) : Math.round(x)} ${units[i]}`;
}

/** Plain words and the status token for any state a computer or run can be in. */
export function statusOf(s: string): { word: string; tone: 'live' | 'working' | 'needs' | 'failed' | 'asleep' } {
	switch (s) {
		case 'awake':
			return { word: 'Awake', tone: 'live' };
		case 'succeeded':
			return { word: 'Done', tone: 'live' };
		case 'active':
			return { word: 'On', tone: 'live' };
		case 'working':
		case 'running':
		case 'starting':
			return { word: 'Working', tone: 'working' };
		case 'waking':
			return { word: 'Waking up', tone: 'working' };
		case 'queued':
		case 'waiting':
			return { word: 'Waiting', tone: 'working' };
		case 'needs_you':
			return { word: 'Needs you', tone: 'needs' };
		case 'held':
			return { word: 'Paused at cap', tone: 'needs' };
		case 'failed':
			return { word: 'Failed', tone: 'failed' };
		case 'timed_out':
			return { word: 'Hit its time limit', tone: 'failed' };
		case 'couldnt_check':
			return { word: "Couldn't check", tone: 'failed' };
		case 'sleeping':
			return { word: 'Going to sleep', tone: 'asleep' };
		case 'stopped':
			return { word: 'Stopped', tone: 'asleep' };
		case 'skipped':
			return { word: 'Skipped', tone: 'asleep' };
		case 'paused':
			return { word: 'Paused', tone: 'asleep' };
		case 'draft':
			return { word: 'Not saved', tone: 'asleep' };
		default:
			return { word: 'Asleep', tone: 'asleep' };
	}
}

export function appName(app: string): string {
	return ({ scripts: 'Scripts', watcher: 'Watcher', code: 'Code', files: 'Files', billing: 'Billing', platform: 'Croncave' } as Record<string, string>)[app] ?? app;
}

export function greeting(name: string, d = new Date()): string {
	const h = d.getHours();
	const part = h < 5 ? 'Good evening' : h < 12 ? 'Good morning' : h < 18 ? 'Good afternoon' : 'Good evening';
	return name ? `${part}, ${name}` : part;
}
