<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import Narrow from '../Narrow.svelte';
	import Button from '$lib/ui/Button.svelte';
	import CardForm from '$lib/ui/CardForm.svelte';
	import PlanCard from '$lib/apps/PlanCard.svelte';
	import { get, post, message } from '$lib/api';
	import { refreshMe, session } from '$lib/session.svelte';
	import { dollars, money } from '$lib/format';

	let step = $state<'phone' | 'code' | 'plan' | 'card' | 'trial' | 'loading'>('loading');
	let phone = $state('');
	let code = $state('');
	let error = $state('');
	let busy = $state(false);
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	let plans = $state<any[]>([]);
	let chosen = $state('');
	let card = $state({ number: '', exp_month: 0, exp_year: 0, cvc: '', zip: '' });
	let award = $state(0);
	let awardNote = $state<string | null>(null);
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	let offer = $state<any>(null);

	onMount(async () => {
		try {
			const me = await refreshMe();
			if (me.signup_step === 'done') return goto('/home', { replaceState: true });
			if (me.signup_step === 'plan') await loadPlans();
			if (me.signup_step === 'trial') offer = (await get('/signup/trial')).offer;
			step = me.signup_step;
		} catch {
			goto('/signin');
		}
	});

	async function loadPlans() {
		plans = (await get('/plans')).plans;
	}

	async function run(fn: () => Promise<void>) {
		busy = true;
		error = '';
		try {
			await fn();
		} catch (e) {
			error = message(e);
		} finally {
			busy = false;
		}
	}

	const sendCode = (e: SubmitEvent) => (e.preventDefault(), run(async () => {
		await post('/auth/phone', { phone });
		step = 'code';
	}));
	const verify = (e: SubmitEvent) => (e.preventDefault(), run(async () => {
		await post('/auth/phone/verify', { code });
		await loadPlans();
		step = 'plan';
	}));

	function pick(id: string) {
		chosen = id;
		const p = plans.find((x) => x.id === id)?.plan;
		if (p?.requires_card) step = 'card';
		else choose();
	}

	const choose = () => run(async () => {
		const r = await post('/signup/plan', { plan: chosen, card: step === 'card' ? card : undefined });
		award = r.award_micros;
		awardNote = r.award_note;
		offer = r.trial;
		step = 'trial';
	});

	const answer = (accept: boolean) => run(async () => {
		await post('/signup/trial', { accept });
		await refreshMe();
		goto(session.me?.computers.length ? '/home' : '/computers/new');
	});
</script>

{#if step === 'plan'}
	<main class="wide">
		<div class="stack">
			<h1>Choose a plan</h1>
			<p class="mid">Every plan comes with free usage. You choose a spending cap, and work pauses there, so you never get a bill you didn't choose.</p>
			{#if error}<div class="error">{error}</div>{/if}
			<div class="plans">
				{#each plans as p (p.id)}
					<PlanCard id={p.id} plan={p.plan} sizes={null}>
						<Button variant={p.id === 'plus' ? 'primary' : 'ghost'} onclick={() => pick(p.id)} {busy}>
							{p.plan.price_monthly ? `Start ${p.plan.name}` : 'Start free'}
						</Button>
					</PlanCard>
				{/each}
			</div>
		</div>
	</main>
{:else}
	<Narrow step={step === 'phone' || step === 'code' ? 'Step 1 of 3' : step === 'card' ? 'Step 2 of 3' : step === 'trial' ? 'Step 3 of 3' : ''}>
		{#if step === 'phone'}
			<h1>Verify your phone</h1>
			<p class="mid">We text you a code. A verified phone keeps sign-up awards fair: one per person. US numbers only for now.</p>
			<form class="stack" onsubmit={sendCode}>
				<input bind:value={phone} placeholder="(415) 555-0123" inputmode="tel" aria-label="Mobile number" autocomplete="tel" />
				{#if error}<div class="error">{error}</div>{/if}
				<Button variant="primary" type="submit" {busy}>Text me a code</Button>
			</form>
		{:else if step === 'code'}
			<h1>Enter the code</h1>
			<p class="mid">We texted a 6-digit code to {phone}. (Simulated: it's in the <a href="/dev" target="_blank">outbox</a>.)</p>
			<form class="stack" onsubmit={verify}>
				<input bind:value={code} inputmode="numeric" maxlength="6" class="mono" aria-label="Code" autocomplete="one-time-code" />
				{#if error}<div class="error">{error}</div>{/if}
				<Button variant="primary" type="submit" {busy}>Verify</Button>
				<Button variant="quiet" onclick={() => (step = 'phone')}>Use another number</Button>
			</form>
		{:else if step === 'card'}
			{@const p = plans.find((x) => x.id === chosen)?.plan}
			<h1>Start {p?.name}</h1>
			<p class="mid">{dollars(p?.price_monthly ?? 0)} a month, charged today. Your spending cap starts at your monthly allowance, so usage never surprises you.</p>
			<CardForm bind:card />
			{#if error}<div class="error" data-testid="card-error">{error}</div>{/if}
			<div class="row"><Button variant="primary" onclick={choose} {busy}>Pay {dollars(p?.price_monthly ?? 0)} and continue</Button><Button variant="quiet" onclick={() => (step = 'plan')}>Back to plans</Button></div>
		{:else if step === 'trial'}
			{#if award}
				<div class="award" data-testid="award">
					<div class="label">Sign-up award</div>
					<div class="big mono">{money(award)}</div>
					<p class="mid">of free usage, spent before your monthly allowance.</p>
				</div>
			{:else if awardNote}
				<p class="mid">{awardNote}</p>
			{/if}
			{#if offer}
				<h1>Try {offer.name} free for {offer.days} days</h1>
				<p class="mid">
					{offer.name} gives you {offer.details.computers} computers, up to {offer.details.largest_size} size, and runs jobs as often as every {offer.details.min_schedule_secs >= 3600 ? 'hour' : `${offer.details.min_schedule_secs / 60} minutes`}.
					It comes with {money(offer.allowance_micros)} of usage for the trial. When it ends you go back to your plan. <strong>Nothing is charged automatically.</strong>
				</p>
				{#if error}<div class="error">{error}</div>{/if}
				<div class="row"><Button variant="primary" onclick={() => answer(true)} {busy}>Start the free trial</Button><Button onclick={() => answer(false)} {busy}>No thanks</Button></div>
			{:else}
				<h1>You're all set</h1>
				<Button variant="primary" onclick={() => answer(false)} {busy}>Create your first computer</Button>
			{/if}
		{:else}
			<p class="mid">Loading…</p>
		{/if}
	</Narrow>
{/if}

<style>
	.wide {
		max-width: 1100px;
		margin: 6vh auto;
		padding: 0 16px;
	}
	.plans {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(230px, 1fr));
		gap: 14px;
	}
	.award {
		background: var(--accent-soft);
		border-radius: 12px;
		padding: 16px;
		text-align: center;
	}
	.big {
		font-size: 34px;
		font-weight: 500;
		color: var(--accent-ink);
	}
</style>
