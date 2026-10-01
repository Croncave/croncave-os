<script lang="ts">
	import type { Snippet } from 'svelte';
	type Props = {
		variant?: 'primary' | 'ghost' | 'danger' | 'quiet';
		size?: 'sm' | 'md';
		type?: 'button' | 'submit';
		disabled?: boolean;
		busy?: boolean;
		href?: string;
		title?: string;
		onclick?: (e: MouseEvent) => void;
		children: Snippet;
	};
	let { variant = 'ghost', size = 'md', type = 'button', disabled = false, busy = false, href, title, onclick, children }: Props = $props();
</script>

{#if href}
	<a class="btn {variant} {size}" {href} {title}>{@render children()}</a>
{:else}
	<button class="btn {variant} {size}" {type} {title} disabled={disabled || busy} {onclick}>
		{#if busy}<span class="spin" aria-hidden="true"></span>{/if}{@render children()}
	</button>
{/if}

<style>
	.btn {
		display: inline-flex;
		align-items: center;
		gap: 8px;
		border-radius: 8px;
		border: 1px solid var(--line);
		background: var(--surface);
		color: var(--ink);
		font: inherit;
		font-weight: 500;
		cursor: pointer;
		white-space: nowrap;
		text-decoration: none;
	}
	.btn:hover:not(:disabled) {
		background: var(--pane);
		text-decoration: none;
	}
	.md {
		padding: 7px 14px;
	}
	.sm {
		padding: 3px 10px;
		font-size: 13px;
	}
	.primary {
		background: var(--accent);
		border-color: var(--accent);
		color: var(--on-accent);
	}
	.primary:hover:not(:disabled) {
		background: var(--accent);
		filter: brightness(1.08);
	}
	.danger {
		color: var(--failed);
	}
	.quiet {
		border-color: transparent;
		background: transparent;
		color: var(--mid);
	}
	.btn:disabled {
		opacity: 0.5;
		cursor: default;
	}
	.spin {
		width: 12px;
		height: 12px;
		border: 2px solid currentColor;
		border-right-color: transparent;
		border-radius: 50%;
		animation: s 0.8s linear infinite;
	}
	@keyframes s {
		to {
			transform: rotate(360deg);
		}
	}
</style>
