<script lang="ts">
	import type { Snippet } from 'svelte';
	// `group` is for fields holding several controls (radios, a row of inputs): a labelled
	// group instead of a <label>, so each control keeps its own name.
	let { label, help, group = false, children }: { label: string; help?: string; group?: boolean; children: Snippet } = $props();
</script>

{#if group}
	<div class="field" role="group" aria-label={label}>
		<span class="name">{label}</span>
		{@render children()}
		{#if help}<span class="help">{help}</span>{/if}
	</div>
{:else}
	<label class="field">
		<span class="name">{label}</span>
		{@render children()}
		{#if help}<span class="help">{help}</span>{/if}
	</label>
{/if}

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
