<script lang="ts">
	// The mock checkout's card form. With real Stripe this becomes Stripe's own card field,
	// so card numbers never reach Croncave.
	let { card = $bindable() }: { card: { number: string; exp_month: number; exp_year: number; cvc: string; zip: string } } = $props();
	let exp = $state('12 / 30');
	$effect(() => {
		const [m, y] = exp.split('/').map((s) => parseInt(s.trim(), 10));
		card.exp_month = m || 0;
		card.exp_year = y ? (y < 100 ? 2000 + y : y) : 0;
	});
</script>

<div class="stack" style="gap: 8px">
	<input bind:value={card.number} inputmode="numeric" autocomplete="cc-number" placeholder="Card number" aria-label="Card number" class="mono" />
	<div class="row" style="flex-wrap: nowrap">
		<input bind:value={exp} placeholder="MM / YY" aria-label="Expiry" class="mono" />
		<input bind:value={card.cvc} placeholder="CVC" aria-label="Security code" class="mono" />
		<input bind:value={card.zip} placeholder="ZIP" aria-label="ZIP code" class="mono" />
	</div>
	<span class="low">Test mode: 4242 4242 4242 4242 works; 4000 0000 0000 0002 is declined.</span>
</div>
