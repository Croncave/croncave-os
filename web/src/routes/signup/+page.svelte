<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import SignupFrame from '$lib/auth/SignupFrame.svelte';
	import Button from '$lib/ui/Button.svelte';
	import CardForm from '$lib/ui/CardForm.svelte';
	import CodeInput from '$lib/ui/CodeInput.svelte';
	import Icon from '$lib/ui/Icon.svelte';
	import Modal from '$lib/ui/Modal.svelte';
	import TimeZonePicker from '$lib/ui/TimeZonePicker.svelte';
	import PlanCard from '$lib/apps/PlanCard.svelte';
	import UsageExplainer from '$lib/apps/UsageExplainer.svelte';
	import { get, post, message } from '$lib/api';
	import { refreshMe, session } from '$lib/session.svelte';
	import { dollars, money } from '$lib/format';
	import { compare, type CatalogPlan } from '$lib/plans';

	// After the email link: your details and phone, the code, a plan, then the trial offer.
	let step = $state<'details' | 'code' | 'plan' | 'trial' | 'loading'>('loading');
	let name = $state('');
	let zone = $state('America/New_York');
	let phone = $state('');
	let agreed = $state(true);
	let code = $state('');
	let error = $state('');
	let busy = $state(false);
	let resendIn = $state(0);
	let timer: ReturnType<typeof setInterval> | undefined;
	let plans = $state<{ id: string; plan: CatalogPlan }[]>([]);
	let chosen = $state('');
	let paying = $state(false);
	let card = $state({ number: '', exp_month: 0, exp_year: 0, cvc: '', zip: '' });
	let award = $state(0);
	let awardNote = $state<string | null>(null);
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	let offer = $state<any>(null);
	let explainer = $state(false);

	const email = $derived(session.me?.user.email ?? '');
	const myPlan = $derived(plans.find((p) => p.id === (chosen || session.me?.account.plan))?.plan);

	onMount(async () => {
		try {
			const me = await refreshMe();
			if (me.signup_step === 'done') return goto(me.computers.length ? '/home' : '/welcome', { replaceState: true });
			name = me.user.name.startsWith('User-') ? '' : me.user.name;
			zone = me.user.time_zone || guessZone();
			if (me.signup_step !== 'phone') await loadPlans();
			if (me.signup_step === 'trial') offer = (await get('/signup/trial')).offer;
			step = me.signup_step === 'phone' ? 'details' : me.signup_step;
			if (step === 'trial' && !offer) finish(false);
		} catch {
			goto('/signin');
		}
	});
	onDestroy(() => clearInterval(timer));

	// The browser's zone when it's one we offer; Eastern otherwise.
	function guessZone() {
		const z = Intl.DateTimeFormat().resolvedOptions().timeZone;
		const ours = ['America/New_York', 'America/Chicago', 'America/Denver', 'America/Phoenix', 'America/Los_Angeles', 'America/Anchorage', 'America/Adak', 'Pacific/Honolulu', 'America/Puerto_Rico', 'Pacific/Pago_Pago', 'Pacific/Guam'];
		return ours.includes(z) ? z : 'America/New_York';
	}

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

	function countdown() {
		resendIn = 30;
		clearInterval(timer);
		timer = setInterval(() => {
			resendIn = Math.max(0, resendIn - 1);
			if (!resendIn) clearInterval(timer);
		}, 1000);
	}

	const sendCode = (e?: SubmitEvent) => (e?.preventDefault(), run(async () => {
		if (!agreed) throw new Error('Agree to the Terms to continue.');
		await post('/auth/phone', { phone, name, time_zone: zone });
		code = '';
		step = 'code';
		countdown();
	}));
	const verify = (e?: SubmitEvent) => (e?.preventDefault(), run(async () => {
		await post('/auth/phone/verify', { code });
		await loadPlans();
		step = 'plan';
	}));

	function pick(id: string) {
		chosen = id;
		error = '';
		if (plans.find((x) => x.id === id)?.plan.requires_card) paying = true;
		else choose();
	}
	const choose = () => run(async () => {
		const r = await post('/signup/plan', { plan: chosen, card: paying ? card : undefined });
		paying = false;
		award = r.award_micros;
		awardNote = r.award_note;
		offer = r.trial;
		await refreshMe();
		step = 'trial';
		if (!offer) finish(false);
	});
	const finish = (accept: boolean) => run(async () => {
		await post('/signup/trial', { accept });
		await refreshMe();
		goto(session.me?.computers.length ? '/home' : '/welcome');
	});

	const fmtPhone = $derived(phone.replace(/\D/g, '').slice(-10).replace(/(\d{3})(\d{3})(\d{4})/, '+1 ($1) $2-$3'));
	const trialEnds = $derived(offer ? new Date(Date.now() + offer.days * 86400_000).toLocaleDateString('en-US', { month: 'short', day: 'numeric' }) : '');
	const clock = (s: number) => `0:${String(s).padStart(2, '0')}`;
	function under(id: string, p: CatalogPlan) {
		if (!p.trial) return '';
		const t = plans.find((x) => x.id === p.trial.plan)?.plan;
		return id === 'free' ? `Includes ${p.trial.days} days of ${t?.name} to try` : `You can try ${t?.name} for ${p.trial.days} days after`;
	}
</script>

<svelte:head><title>Sign up · Croncave</title></svelte:head>

{#if step === 'details' || step === 'code'}
	<SignupFrame {email} step={1}>
		{#if step === 'details'}
			<form class="card" onsubmit={sendCode}>
				<div class="head">
					<h1>Your details and phone</h1>
					<p class="lede">Your email is confirmed. Next we’ll text your phone a code.</p>
				</div>
				<label class="field"><span class="name">Your name</span><input bind:value={name} autocomplete="name" /></label>
				<div class="field"><span class="name">Time zone</span><TimeZonePicker bind:value={zone} /></div>
				<div class="field">
					<label class="name" for="mobile">Mobile number</label>
					<div class="phone"><span class="cc">US <span class="mono">+1</span></span><input id="mobile" class="mono" bind:value={phone} placeholder="(555) 014-2297" inputmode="tel" autocomplete="tel-national" aria-label="Mobile number" /></div>
					<span class="help">US numbers only. We text you a code now, and alerts later only if you turn them on.</span>
				</div>
				<label class="agree">
					<input type="checkbox" bind:checked={agreed} class="sr" />
					<span class="box" class:on={agreed}>{#if agreed}<Icon name="check" size={13} stroke={2.4} />{/if}</span>
					<span>I agree to the Terms and Privacy Policy, and I’m in the US.</span>
				</label>
				{#if error}<div class="error">{error}</div>{/if}
				<button class="go" type="submit" disabled={busy}>Send code</button>
			</form>
		{:else}
			<form class="card narrow" onsubmit={verify}>
				<div class="head">
					<h1>Confirm your phone</h1>
					<p class="lede">We sent a 6-digit code to <span class="mono strong">{fmtPhone}</span>. It's only used to keep sign-ups fair and to text you alerts if you turn them on.</p>
				</div>
				<CodeInput bind:value={code} oncomplete={() => verify()} />
				<div class="between">
					{#if resendIn > 0}<span>Resend code in <span class="mono strong">{clock(resendIn)}</span></span>
					{:else}<button type="button" class="linkish" onclick={() => sendCode()}>Resend code</button>{/if}
					<button type="button" class="linkish" onclick={() => ((step = 'details'), (error = ''))}>Use a different number</button>
				</div>
				{#if error}<div class="error">{error}</div>{/if}
				<button class="go" type="submit" disabled={busy || code.length < 6}>Confirm</button>
				<div class="info"><span class="i"><Icon name="info" size={15} /></span><span>Each phone number gets one sign-up award and one free trial. Using the same number on another account won't add another.</span></div>
			</form>
		{/if}
	</SignupFrame>
{:else if step === 'plan'}
	<SignupFrame {email} step={2}>
		<div class="plans-head">
			<h1 class="big">Choose a plan</h1>
			<p class="lede wide">Plans set what your computers can do, and each one comes with free usage: the time your computers are awake and the storage they keep. Computers sleep when nothing is running, so free usage goes a long way.</p>
		</div>
		{#if error && !paying}<div class="error">{error}</div>{/if}
		<div class="plans">
			{#each plans as p (p.id)}
				<PlanCard id={p.id} plan={p.plan} note={under(p.id, p.plan)}>
					<button class="choose" class:primary={p.plan.recommended} onclick={() => pick(p.id)} disabled={busy}>
						{p.plan.price_monthly ? `Choose ${p.plan.name}` : 'Start free'}
					</button>
				</PlanCard>
			{/each}
		</div>
		<div class="foot">
			<span><Icon name="pause" size={14} />When your free usage runs out, work pauses. Nothing is charged past your plan unless you turn on overage.</span>
			<span><Icon name="clock" size={14} />Change or cancel anytime.</span>
			<button class="linkish" onclick={() => (explainer = true)}>What counts as usage?</button>
		</div>
	</SignupFrame>
	{#if paying && myPlan}
		<Modal title="Start {myPlan.name}" lede="{dollars(myPlan.price_monthly)} a month, charged today. Your spending cap starts at your monthly allowance, so usage never surprises you." onclose={() => (paying = false)}>
			<CardForm bind:card />
			{#if error}<div class="error" data-testid="card-error">{error}</div>{/if}
			<div class="actions"><Button onclick={() => (paying = false)}>Back to plans</Button><Button variant="primary" onclick={choose} {busy}>Pay {dollars(myPlan.price_monthly)} and continue</Button></div>
		</Modal>
	{/if}
	{#if explainer}<UsageExplainer onclose={() => (explainer = false)} />{/if}
{:else if step === 'trial' && offer && myPlan}
	{@const to = plans.find((p) => p.id === offer.plan)?.plan ?? offer.details}
	<SignupFrame {email} step={3} steps={false}>
		<div class="behind" aria-hidden="true">
			<h1 class="big">Set up your first computer</h1>
			<div class="ghosts"><div></div><div></div><div></div></div>
		</div>
	</SignupFrame>
	<Modal
		title="Try {offer.name} free for {offer.days} days"
		eyebrow={award ? `You’re on ${myPlan.name}, with ${money(award)} of free usage to start` : `You’re on ${myPlan.name}`}
		lede="See what your watches and scripts can do with faster schedules and a bigger computer."
		closable={false}
		onclose={() => {}}
	>
		<div class="compare" data-testid="award">
			<div class="crow headrow"><span></span><span>{myPlan.name}</span><span>{offer.name} trial</span></div>
			{#each compare(myPlan, to, money(offer.allowance_micros), award ? money(award) : 'None') as r (r.label)}
				<div class="crow"><span class="mid">{r.label}</span><span class="mid">{r.from}</span><span class="strong-cell">{r.to}</span></div>
			{/each}
		</div>
		{#if awardNote}<p class="mid small">{awardNote}</p>{/if}
		<div class="ticks">
			<div><span class="check"><Icon name="check" size={14} stroke={2} /></span><span>No card needed.</span></div>
			<div><span class="check"><Icon name="check" size={14} stroke={2} /></span><span>On <span class="mono strong">{trialEnds}</span> you go back to {myPlan.name}, unless you choose to keep {offer.name}. Nothing is charged automatically.</span></div>
			<div><span class="check"><Icon name="check" size={14} stroke={2} /></span><span>We’ll remind you 2 days before it ends.</span></div>
		</div>
		{#if error}<div class="error">{error}</div>{/if}
		<div class="actions"><Button onclick={() => finish(false)} {busy}>Stay on {myPlan.name}</Button><Button variant="primary" onclick={() => finish(true)} {busy}>Start my {offer.name} trial</Button></div>
	</Modal>
{:else}
	<div class="loading mid">Loading…</div>
{/if}

<style>
	.card {
		width: 500px;
		max-width: 100%;
		background: var(--surface);
		border: 1px solid var(--line);
		border-radius: 14px;
		padding: 28px;
		display: flex;
		flex-direction: column;
		gap: 18px;
	}
	.card.narrow {
		width: 460px;
		gap: 20px;
	}
	.head {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}
	h1 {
		font-size: 22px;
		line-height: 30px;
		font-weight: 600;
	}
	h1.big {
		font-size: 28px;
		line-height: 36px;
		letter-spacing: -0.02em;
		text-align: center;
	}
	.lede {
		font-size: 14px;
		line-height: 21px;
		color: var(--mid);
	}
	.lede.wide {
		max-width: 660px;
		text-align: center;
		font-size: 15px;
		line-height: 22px;
	}
	.strong {
		color: var(--ink);
	}
	.field {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}
	.name {
		font-size: 13px;
		font-weight: 500;
	}
	.field input {
		height: 42px;
	}
	.phone {
		display: flex;
		gap: 8px;
	}
	.cc {
		height: 42px;
		padding: 0 12px;
		border: 1px solid var(--line);
		border-radius: 6px;
		background: var(--pane);
		display: flex;
		align-items: center;
		gap: 6px;
		font-size: 14px;
		color: var(--mid);
		flex-shrink: 0;
	}
	.cc .mono {
		color: var(--ink);
		font-size: 14px;
	}
	.help {
		font-size: 12px;
		line-height: 16px;
		color: var(--mid);
	}
	.agree {
		display: flex;
		gap: 10px;
		align-items: flex-start;
		font-size: 13px;
		line-height: 19px;
		color: var(--mid);
		cursor: pointer;
		position: relative;
	}
	.sr {
		position: absolute;
		opacity: 0;
		width: 1px;
		height: 1px;
	}
	.box {
		width: 18px;
		height: 18px;
		border-radius: 4px;
		border: 1px solid var(--line-strong);
		display: flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
	}
	.box.on {
		background: var(--accent);
		border-color: var(--accent);
		color: var(--on-accent);
	}
	.agree:focus-within .box {
		outline: 2px solid var(--focus);
		outline-offset: 2px;
	}
	.go {
		height: 44px;
		border-radius: 8px;
		border: 0;
		background: var(--accent);
		color: var(--on-accent);
		font: 600 15px Inter, system-ui, sans-serif;
		cursor: pointer;
	}
	.go:disabled {
		opacity: 0.55;
		cursor: default;
	}
	.between {
		display: flex;
		justify-content: space-between;
		font-size: 13px;
		color: var(--mid);
	}
	.linkish {
		background: none;
		border: 0;
		padding: 0;
		font: inherit;
		font-size: 13px;
		color: var(--accent-ink);
		cursor: pointer;
	}
	.linkish:hover {
		color: var(--accent-hover);
	}
	.info {
		display: flex;
		gap: 10px;
		padding: 12px 14px;
		border-radius: 8px;
		background: var(--pane);
		border: 1px solid var(--line);
		font-size: 13px;
		line-height: 19px;
		color: var(--mid);
	}
	.i {
		display: flex;
		padding-top: 2px;
	}
	.plans-head {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 8px;
	}
	.plans {
		display: grid;
		grid-template-columns: repeat(4, minmax(0, 1fr));
		gap: 16px;
		width: min(1240px, 100%);
		padding-top: 8px;
	}
	.choose {
		height: 42px;
		border-radius: 8px;
		font: 600 14px Inter, system-ui, sans-serif;
		background: var(--surface);
		color: var(--ink);
		border: 1px solid var(--line-strong);
		cursor: pointer;
	}
	.choose.primary {
		background: var(--accent);
		border-color: var(--accent);
		color: var(--on-accent);
	}
	.foot {
		display: flex;
		flex-wrap: wrap;
		justify-content: center;
		gap: 12px 28px;
		font-size: 13px;
		color: var(--mid);
		padding-top: 4px;
	}
	.foot span {
		display: flex;
		align-items: center;
		gap: 6px;
	}
	.actions {
		display: flex;
		justify-content: flex-end;
		gap: 8px;
		padding-top: 4px;
	}
	.behind {
		opacity: 0.5;
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 24px;
	}
	.ghosts {
		display: grid;
		grid-template-columns: repeat(3, minmax(0, 1fr));
		gap: 14px;
		width: min(820px, 90vw);
	}
	.ghosts div {
		height: 150px;
		border-radius: 12px;
		border: 1px solid var(--line);
		background: var(--surface);
	}
	.compare {
		border: 1px solid var(--line);
		border-radius: 10px;
		background: var(--pane);
	}
	.crow {
		display: grid;
		grid-template-columns: minmax(0, 1.3fr) minmax(0, 1fr) minmax(0, 1fr);
		gap: 12px;
		padding: 10px 14px;
		font-size: 13px;
	}
	.crow + .crow {
		border-top: 1px solid var(--line);
	}
	.headrow span {
		font: 500 11px/16px 'JetBrains Mono', ui-monospace, monospace;
		letter-spacing: 0.08em;
		text-transform: uppercase;
		color: var(--low);
	}
	.strong-cell {
		font-weight: 600;
	}
	.ticks {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}
	.ticks div {
		display: flex;
		gap: 8px;
		align-items: flex-start;
		font-size: 13px;
		line-height: 19px;
	}
	.check {
		color: var(--accent-ink);
		display: flex;
		padding-top: 2px;
	}
	.ticks .mono {
		font-size: 13px;
	}
	.small {
		font-size: 13px;
	}
	.loading {
		padding: 20vh 0;
		text-align: center;
	}
	@media (max-width: 1100px) {
		.plans {
			grid-template-columns: repeat(2, minmax(0, 1fr));
			gap: 24px 16px;
		}
	}
	@media (max-width: 560px) {
		.plans {
			grid-template-columns: minmax(0, 1fr);
		}
	}
</style>
