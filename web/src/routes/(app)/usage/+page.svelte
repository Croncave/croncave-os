<script lang="ts">
	import { get } from '$lib/api';
	import { appName, duration, money } from '$lib/format';
	import Chart from '$lib/ui/Chart.svelte';

	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	let b = $state<any>(null);
	$effect(() => {
		get('/billing').then((r) => (b = r));
	});
	const byDay = $derived.by(() => {
		if (!b) return [];
		const days = new Map<string, number>();
		for (const r of b.usage_by_day) days.set(r.day, (days.get(r.day) ?? 0) + r.cost_micros);
		return [...days.entries()].map(([d, c], i) => ({ x: i, y: c / 1e6, d }));
	});
</script>

<div class="page">
	<h1>Usage</h1>
	{#if b}
		<p class="mid">This month so far: <span class="mono">{money(b.used_this_period_micros)}</span>, including <span class="mono">{money(b.ai_cost_micros)}</span> of assistant AI. Shown in plain units: dollars, hours awake and GB stored.</p>
		<section class="card stack"><h2>By day</h2><Chart points={byDay} kind="bar" format={(v) => `$${v.toFixed(4)}`} /></section>
		<div class="grid2">
			<section class="card stack">
				<h2>By computer</h2>
				<table>
					<thead><tr><th>Computer</th><th>What</th><th>Amount</th><th>Cost</th></tr></thead>
					<tbody>
						{#each b.usage_by_computer as u, i (i)}
							<tr><td>{u.name}</td><td>{u.meter === 'compute' ? 'Awake' : 'Stored'}</td><td class="mono">{u.meter === 'compute' ? duration(u.quantity) : `${u.quantity.toFixed(4)} GB-hours`}</td><td class="mono">{money(u.cost_micros)}</td></tr>
						{:else}<tr><td colspan="4" class="low">No usage yet.</td></tr>{/each}
					</tbody>
				</table>
			</section>
			<section class="card stack">
				<h2>By app</h2>
				<table>
					<thead><tr><th>App</th><th>Runs</th><th>Running time</th></tr></thead>
					<tbody>
						{#each b.usage_by_app as u (u.app)}<tr><td>{appName(u.app)}</td><td class="mono">{u.runs}</td><td class="mono">{duration(u.awake_seconds)}</td></tr>{:else}<tr><td colspan="3" class="low">No runs yet.</td></tr>{/each}
					</tbody>
				</table>
			</section>
		</div>
	{/if}
</div>
