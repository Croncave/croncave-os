export const toasts = $state({ items: [] as { id: number; text: string; bad: boolean }[] });
let next = 1;

export function toast(text: string, bad = false) {
	const id = next++;
	toasts.items.push({ id, text, bad });
	setTimeout(() => (toasts.items = toasts.items.filter((t) => t.id !== id)), bad ? 7000 : 3500);
}
