<script lang="ts">
	import { get, post, message } from '$lib/api';
	import { onLive, throttle } from '$lib/live';
	import { refreshMe } from '$lib/session.svelte';
	import { dollars, money, when } from '$lib/format';
	import Button from '$lib/ui/Button.svelte';
	import Modal from '$lib/ui/Modal.svelte';
	import CardForm from '$lib/ui/CardForm.svelte';
	import PlanCard from '$lib/apps/PlanCard.svelte';
	import { toast } from '$lib/ui/toast.svelte';

	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	let b = $state<any>(null);
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	let plans = $state<any[]>([]);
	let cap = $state('');
	let overage = $state(false);
	let changing = $state<string>('');
	let card = $state({ number: '', exp_month: 0, exp_year: 0, cvc: '', zip: '' });
	let busy = $state(false);
	let error = $state('');
	let promo = $state('');

	async function load() {
		b = await get('/billing');
		plans = (await get('/plans')).plans;
		cap = (b.cap.micros / 1e6).toFixed(2);
		overage = b.cap.overage_enabled;
	}
	$effect(() => {
		load();
	});
	$effect(() => onLive(throttle(load, 800)));

	async function saveCap() {
		try {
			const r = await post('/billing/cap', { cap_dollars: Number(cap), overage_enabled: overage });
			toast(r.resumed ? 'Saved. Work resumed.' : 'Saved');
			load();
			refreshMe();
		} catch (e) {
			toast(message(e), true);
		}
	}
	async function changePlan() {
		busy = true;
		error = '';
		try {
			const needsCard = plans.find((p) => p.id === changing)?.plan.requires_card && !b.card;
			await post('/billing/plan', { plan: changing, card: needsCard || card.number ? card : undefined });
			toast('Plan changed');
			changing = '';
			load();
			refreshMe();
		} catch (e) {
			error = message(e);
		} finally {
			busy = false;
		}
	}
	async function redeem() {
		try {
			const r = await post('/billing/promo', { code: promo });
			toast(`Added ${money(r.credit_micros)} of credit`);
			promo = '';
			load();
			refreshMe();
		} catch (e) {
			toast(message(e), true);
		}
	}
	const pct = (n: number, d: number) => Math.min(100, d > 0 ? (n / d) * 100 : 0);
	const daysLeft = (iso: string) => Math.max(0, Math.ceil((new Date(iso).getTime() - Date.now()) / 86400000));
</script>

<div class="page">
	<h1>Plans and billing</h1>
	{#if b}
		{#if b.paused}
			<div class="banner bad" data-testid="usage-ran-out">
				<div class="stack" style="gap: 2px">
					<strong>Your usage ran out, so work is paused.</strong>
					<span class="mid">{b.cap.overage_allowed ? 'Raise your cap or turn on overage to keep going. ' : `${b.effective_plan.name} pauses at its allowance so it never becomes a bill. A paid plan lets you opt in to more. `}Nothing is deleted.</span>
				</div>
			</div>
		{/if}
		{#if b.trial}
			<div class="banner" data-testid="trial-banner">
				<div class="stack" style="gap: 2px">
					<strong>You're trying {b.trial.name}: {daysLeft(b.trial.ends_at)} days left.</strong>
					<span class="mid">On {when(b.trial.ends_at)} you go back to {b.plan?.name}. Nothing is charged automatically.</span>
				</div>
				<span class="spacer"></span>
				<Button variant="primary" onclick={() => (changing = b.trial.plan)}>Keep {b.trial.name}</Button>
			</div>
		{/if}
		<div class="grid2">
			<section class="card stack">
				<div class="label">Your plan</div>
				<div class="row"><h2>{b.plan?.name}</h2>{#if b.plan?.price_monthly}<span class="mono">{dollars(b.plan.price_monthly)}/month</span>{:else}<span class="low">free</span>{/if}</div>
				<div class="low">This month: {when(b.period.start)} to {when(b.period.end)}</div>
				{#if b.card}<div class="low">Card: {b.card.brand} ending {b.card.last4}</div>{/if}
			</section>
			<section class="card stack" data-testid="balances">
				<div class="label">Usage left</div>
				<div class="big mono">{money(b.left_micros)}</div>
				<div class="list small">
					{#if b.balances.award > 0}<div class="row"><span>Sign-up award</span><span class="spacer"></span><span class="mono">{money(b.balances.award)}</span></div>{/if}
					{#if b.balances.credit > 0}<div class="row"><span>Credits</span><span class="spacer"></span><span class="mono">{money(b.balances.credit)}</span></div>{/if}
					{#if b.balances.trial > 0}<div class="row"><span>Trial allowance</span><span class="spacer"></span><span class="mono">{money(b.balances.trial)}</span></div>{/if}
					<div class="row"><span>Monthly allowance</span><span class="spacer"></span><span class="mono">{money(Math.max(0, b.balances.allowance))}</span></div>
					{#if b.balances.overage > 0}<div class="row"><span>Overage so far (billed at month end)</span><span class="spacer"></span><span class="mono">{money(b.balances.overage)}</span></div>{/if}
				</div>
				<span class="low">Usage draws from your award first, then your monthly allowance, then overage if you turned it on.</span>
			</section>
		</div>
		<section class="card stack">
			<h2>Spending cap</h2>
			<div class="meter" aria-label="Spent against the cap">
				<span style="width: {pct(b.balances.cap_spend, b.cap.micros)}%"></span>
				<i style="left: 50%"></i><i style="left: 80%"></i>
			</div>
			<div class="row low mono"><span>{money(b.balances.cap_spend)} of {money(b.cap.micros)} this month</span><span class="spacer"></span><span>alerts at 50%, 80% and 100%</span></div>
			{#if b.cap.overage_allowed}
				<div class="row" style="flex-wrap: nowrap">
					<label class="row"><input type="checkbox" bind:checked={overage} data-testid="overage" /> Keep working past my allowance (overage)</label>
					<span class="spacer"></span>
					<label class="row">Cap $<input class="mono capin" bind:value={cap} inputmode="decimal" aria-label="Cap in dollars" /></label>
					<Button onclick={saveCap}>Save</Button>
				</div>
				<span class="low">At the cap, computers finish what they're doing and sleep. Nothing is deleted, and raising the cap resumes work straight away.</span>
			{:else}
				<p class="mid">{b.effective_plan.name} pauses at its allowance, so a free account never becomes a bill.</p>
			{/if}
		</section>
		<section class="stack">
			<h2>Plans</h2>
			<div class="plans">
				{#each plans as p (p.id)}
					<PlanCard id={p.id} plan={p.plan} sizes={null} current={p.id === b.plan_id}>
						{#if p.id !== b.plan_id}
							<Button size="sm" variant={p.id === b.trial?.plan ? 'primary' : 'ghost'} onclick={() => (changing = p.id)}>{p.id === b.trial?.plan ? `Keep ${p.plan.name}` : `Switch to ${p.plan.name}`}</Button>
						{/if}
					</PlanCard>
				{/each}
			</div>
		</section>
		<div class="grid2">
			<section class="card stack">
				<h2>Invoices</h2>
				<div class="list">
					{#each b.invoices as inv (inv.id)}
						<a class="row" href="/invoices/{inv.id}"><span>{when(inv.period_start)}</span><span class="chip">{inv.status}</span><span class="spacer"></span><span class="mono">{money(inv.total_micros)}</span></a>
					{:else}<div class="low">No invoices yet.</div>{/each}
				</div>
			</section>
			<section class="card stack">
				<h2>Promo code</h2>
				<div class="row" style="flex-wrap: nowrap"><input bind:value={promo} placeholder="ALPHA5" class="mono" aria-label="Promo code" /><Button onclick={redeem} disabled={!promo}>Apply</Button></div>
				<h3>Recent credits and grants</h3>
				<div class="list small">
					{#each b.grants as g, i (i)}<div class="row"><span>{g.reason || g.kind}</span><span class="spacer"></span><span class="mono">{money(g.amount_micros)}</span></div>{/each}
				</div>
			</section>
		</div>
	{/if}
</div>

{#if changing}
	{@const p = plans.find((x) => x.id === changing)?.plan}
	<Modal title={b?.trial?.plan === changing ? `Keep ${p?.name}` : `Switch to ${p?.name}`} onclose={() => (changing = '')}>
		<p class="mid">{p?.price_monthly ? `${dollars(p.price_monthly)} a month, charged today.` : 'Free.'} Changing plans doesn't come with a sign-up award.</p>
		{#if p?.requires_card}
			{#if b?.card}<p class="low">Charged to {b.card.brand} ending {b.card.last4}, or enter another card:</p>{/if}
			<CardForm bind:card />
		{/if}
		{#if error}<div class="error">{error}</div>{/if}
		<div class="row"><Button variant="primary" onclick={changePlan} {busy}>Confirm</Button><Button onclick={() => (changing = '')}>Cancel</Button></div>
	</Modal>
{/if}

<style>
	.big {
		font-size: 28px;
	}
	.small {
		font-size: 13px;
	}
	.plans {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
		gap: 12px;
	}
	.meter {
		position: relative;
		height: 10px;
		background: var(--pane);
		border-radius: 5px;
		overflow: hidden;
	}
	.meter span {
		position: absolute;
		inset: 0 auto 0 0;
		background: var(--accent);
	}
	.meter i {
		position: absolute;
		top: 0;
		bottom: 0;
		width: 1px;
		background: var(--line);
	}
	.capin {
		width: 90px;
	}
</style>
