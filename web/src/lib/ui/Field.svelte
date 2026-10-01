<script lang="ts">
	import type { Snippet } from 'svelte';
	let { label, help, children }: { label: string; help?: string; children: Snippet } = $props();
</script>

<label class="field">
	<span class="name">{label}</span>
	{@render children()}
	{#if help}<span class="help">{help}</span>{/if}
</label>

<style>
	.field {
		display: grid;
		gap: 5px;
	}
	.name {
		font-weight: 500;
		font-size: 13px;
	}
	.help {
		color: var(--low);
		font-size: 12px;
	}
</style>
