<script lang="ts">
	import type { Snippet } from 'svelte';
	import Icon from '$lib/ui/Icon.svelte';
	import { dollars } from '$lib/format';
	import { features, type CatalogPlan } from '$lib/plans';

	// A plan as the sign-up and the plans page draw it; every figure is the catalog's.
	let {
		id,
		plan,
		current = false,
		highlight = plan.recommended,
		badge,
		note,
		children
	}: { id: string; plan: CatalogPlan; current?: boolean; highlight?: boolean; badge?: string; note?: string; children?: Snippet } = $props();
	const tag = $derived(badge ?? (current ? 'Your plan' : highlight ? 'Recommended' : ''));
</script>

<div class="plan" class:hl={highlight || current} data-plan={id}>
	{#if tag}<span class="tag">{tag}</span>{/if}
	<div class="top">
		<span class="name">{plan.name}</span>
		<span class="tagline">{plan.tagline ?? ''}</span>
	</div>
	<div class="price"><span class="mono amount">{plan.price_monthly ? dollars(plan.price_monthly) : '$0'}</span><span class="per">{plan.price_monthly ? 'a month' : 'no card'}</span></div>
	<div class="usage">
		<div class="u1"><span class="gift"><Icon name="gift" size={14} /></span>Starts with <span class="mono strong">{dollars(plan.award)}</span> of free usage</div>
		<div class="u2">then <span class="mono strong">{dollars(plan.allowance)}</span> of free usage every month</div>
	</div>
	<div class="feats">
		{#each features(plan) as f (f)}<div class="f"><span class="check"><Icon name="check" size={14} stroke={2} /></span><span>{f}</span></div>{/each}
	</div>
	{#if children}{@render children()}{/if}
	<span class="note">{note ?? ''}</span>
</div>

<style>
	.plan {
		position: relative;
		display: flex;
		flex-direction: column;
		gap: 16px;
		padding: 22px 20px;
		border-radius: 14px;
		border: 1px solid var(--line);
		background: var(--surface);
		min-width: 0;
	}
	.hl {
		border: 2px solid var(--accent);
		padding: 21px 19px;
	}
	.tag {
		position: absolute;
		top: -12px;
		left: 18px;
		padding: 3px 10px;
		border-radius: 9999px;
		background: var(--accent);
		color: var(--on-accent);
		font-size: 12px;
		font-weight: 600;
	}
	.top {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}
	.name {
		font-size: 17px;
		font-weight: 600;
	}
	.tagline {
		font-size: 13px;
		line-height: 19px;
		color: var(--mid);
		min-height: 38px;
	}
	.price {
		display: flex;
		align-items: baseline;
		gap: 6px;
	}
	.amount {
		font-size: 30px;
		font-weight: 500;
	}
	.per {
		font-size: 13px;
		color: var(--mid);
	}
	.usage {
		display: flex;
		flex-direction: column;
		gap: 6px;
		padding: 12px;
		border-radius: 8px;
		background: var(--pane);
		border: 1px solid var(--line);
	}
	.u1 {
		display: flex;
		align-items: center;
		gap: 8px;
		font-size: 13px;
	}
	.u1 .mono {
		font-size: 13px;
		font-weight: 500;
	}
	.u2 {
		font-size: 12px;
		color: var(--mid);
		padding-left: 22px;
	}
	.u2 .mono {
		font-size: 12px;
	}
	.strong {
		color: var(--ink);
	}
	.gift,
	.check {
		color: var(--accent-ink);
		display: flex;
	}
	.check {
		padding-top: 2px;
	}
	.feats {
		display: flex;
		flex-direction: column;
		gap: 8px;
		flex-grow: 1;
	}
	.f {
		display: flex;
		gap: 8px;
		align-items: flex-start;
		font-size: 13px;
		line-height: 19px;
	}
	.note {
		font-size: 12px;
		color: var(--mid);
		text-align: center;
		min-height: 16px;
		margin-top: -4px;
	}
</style>
