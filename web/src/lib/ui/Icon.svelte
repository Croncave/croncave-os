<script lang="ts">
	import { ICONS } from './icons';

	let { name, size = 16, stroke = 1.6 }: { name: string; size?: number; stroke?: number } = $props();
</script>

<svg
	width={size}
	height={size}
	viewBox="0 0 24 24"
	fill="none"
	stroke="currentColor"
	stroke-width={stroke}
	stroke-linecap="round"
	stroke-linejoin="round"
	aria-hidden="true"
	style="flex-shrink: 0">{@html ICONS[name] ?? ''}</svg
>
