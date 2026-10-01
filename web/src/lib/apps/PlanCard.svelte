<script lang="ts">
	import type { Snippet } from 'svelte';
	import { dollars } from '$lib/format';
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	let { id, plan, current = false, children }: { id: string; plan: any; sizes: unknown; current?: boolean; children?: Snippet } = $props();
	const every = (s: number) => (s >= 3600 ? 'Hourly' : s >= 300 ? `Every ${s / 60} minutes` : 'Every minute');
</script>

<div class="plan card" class:current data-plan={id}>
	<div class="row"><h2>{plan.name}</h2>{#if current}<span class="chip">Your plan</span>{/if}</div>
	<div><span class="price mono">{plan.price_monthly ? dollars(plan.price_monthly) : '$0'}</span> <span class="low">{plan.price_monthly ? 'a month' : 'no card'}</span></div>
	<ul>
		<li><span class="mono">{dollars(plan.award)}</span> sign-up award</li>
		<li><span class="mono">{dollars(plan.allowance)}</span> of usage a month after that</li>
		<li>{plan.computers} computer{plan.computers > 1 ? 's' : ''}, {plan.awake_at_once} awake at a time</li>
		<li>Up to {plan.largest_size[0].toUpperCase() + plan.largest_size.slice(1)}</li>
		<li>{every(plan.min_schedule_secs)} at most</li>
		<li>{plan.keep_awake ? `${plan.keep_awake} keep-awake` : 'Sleeps when idle'}</li>
		<li>{plan.overage ? 'Opt-in overage, with your own cap' : 'Work pauses at the allowance'}</li>
		<li>History kept {plan.history_days} days</li>
		{#if plan.trial}<li class="trial">{plan.trial.days}-day trial of the next plan up</li>{/if}
	</ul>
	{#if children}{@render children()}{/if}
</div>

<style>
	.plan {
		display: grid;
		gap: 10px;
		align-content: start;
	}
	.current {
		border-color: var(--accent);
	}
	.price {
		font-size: 26px;
	}
	ul {
		margin: 0;
		padding-left: 18px;
		color: var(--mid);
		display: grid;
		gap: 3px;
	}
	.trial {
		color: var(--accent-ink);
	}
</style>
