<script lang="ts">
	import type { Snippet } from 'svelte';
	import Icon from './Icon.svelte';
	type Props = {
		variant?: 'primary' | 'ghost' | 'danger' | 'quiet';
		size?: 'xs' | 'sm' | 'md' | 'lg';
		type?: 'button' | 'submit';
		icon?: string;
		block?: boolean;
		disabled?: boolean;
		busy?: boolean;
		href?: string;
		title?: string;
		label?: string;
		onclick?: (e: MouseEvent) => void;
		children?: Snippet;
	};
	let {
		variant = 'ghost',
		size = 'md',
		type = 'button',
		icon,
		block = false,
		disabled = false,
		busy = false,
		href,
		title,
		label,
		onclick,
		children
	}: Props = $props();
	const iconSize = $derived(size === 'xs' ? 13 : size === 'sm' ? 14 : 15);
</script>

{#if href}
	<a class="btn {variant} {size}" class:block {href} {title} aria-label={label}>
		{#if icon}<Icon name={icon} size={iconSize} />{/if}{@render children?.()}
	</a>
{:else}
	<button class="btn {variant} {size}" class:block {type} {title} aria-label={label} disabled={disabled || busy} {onclick}>
		{#if busy}<span class="spin" aria-hidden="true"></span>{:else if icon}<Icon name={icon} size={iconSize} />{/if}{@render children?.()}
	</button>
{/if}

<style>
	.btn {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		gap: 6px;
		height: 36px;
		padding: 0 14px;
		border-radius: 6px;
		border: 1px solid var(--line);
		background: var(--surface);
		color: var(--ink);
		font: 500 14px Inter, system-ui, sans-serif;
		cursor: pointer;
		white-space: nowrap;
		text-decoration: none;
		flex-shrink: 0;
	}
	.btn:hover:not(:disabled) {
		border-color: var(--line-strong);
		text-decoration: none;
	}
	.lg {
		height: 40px;
	}
	.sm {
		height: 30px;
		padding: 0 10px;
		font-size: 13px;
	}
	.xs {
		height: 28px;
		padding: 0 8px;
		font-size: 12px;
	}
	.block {
		flex-grow: 1;
	}
	.primary {
		background: var(--accent);
		border-color: var(--accent);
		color: var(--on-accent);
	}
	.primary:hover:not(:disabled) {
		border-color: var(--accent);
		filter: brightness(1.08);
	}
	.danger {
		color: var(--failed);
		border-color: var(--failed);
	}
	.danger:hover:not(:disabled) {
		border-color: var(--failed);
		background: var(--failed-soft);
	}
	.quiet {
		border-color: transparent;
		background: transparent;
		color: var(--mid);
	}
	.quiet:hover:not(:disabled) {
		border-color: transparent;
		color: var(--ink);
		background: var(--raised);
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
