<script lang="ts">
	import Brand from './Brand.svelte';
	let { children, step }: { children: import('svelte').Snippet; step?: string } = $props();
</script>

<main class="narrow">
	<div class="row"><Brand />{#if step}<span class="spacer"></span><span class="label">{step}</span>{/if}</div>
	<div class="card stack">{@render children()}</div>
	<p class="low foot">Croncave is in private alpha. Payments, email and text messages are simulated; read them on the <a href="/dev">Dev tools</a> page.</p>
</main>

<style>
	.narrow {
		max-width: 480px;
		margin: 8vh auto;
		padding: 0 16px;
		display: grid;
		gap: 16px;
	}
	.foot {
		font-size: 12px;
	}
</style>
