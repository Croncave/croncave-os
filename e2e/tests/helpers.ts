import { expect, type APIRequestContext, type Page } from '@playwright/test';

let counter = 0;
export function unique(prefix = 'user') {
	counter++;
	return `${prefix}-${Date.now().toString(36)}-${counter}@example.com`;
}

/** A US mobile number nobody has used, so awards and trials aren't already claimed. */
export function phone() {
	return `(415) 55${Math.floor(Math.random() * 10)}-${String(Math.floor(Math.random() * 10000)).padStart(4, '0')}`;
}

async function outbox(request: APIRequestContext) {
	const r = await request.get('/api/dev/outbox');
	return (await r.json()).messages as { channel: string; to: string; body: string; link: string | null }[];
}

export async function signInLink(request: APIRequestContext, email: string): Promise<string> {
	await expect.poll(async () => (await outbox(request)).find((m) => m.to === email && m.link)?.link ?? '').toContain('/auth/verify');
	const link = (await outbox(request)).find((m) => m.to === email && m.link)!.link!;
	return link.replace(/^https?:\/\/[^/]+/, '');
}

export async function smsCode(request: APIRequestContext, phoneNumber: string): Promise<string> {
	const digits = '+1' + phoneNumber.replace(/\D/g, '').slice(-10);
	await expect.poll(async () => (await outbox(request)).find((m) => m.channel === 'sms' && m.to === digits)?.body ?? '').toMatch(/\d{6}/);
	return (await outbox(request)).find((m) => m.channel === 'sms' && m.to === digits)!.body.match(/\d{6}/)![0];
}

type SignUp = { email?: string; plan?: 'free' | 'plus' | 'pro' | 'max'; trial?: boolean; card?: string };

/** The whole sign-up through the UI. Returns the email used. */
export async function signUp(page: Page, opts: SignUp = {}): Promise<string> {
	const email = opts.email ?? unique();
	const number = phone();
	await page.goto('/signin');
	await page.getByLabel('Email').fill(email);
	await page.getByRole('button', { name: 'Email me a sign-in link' }).click();
	await expect(page.getByText('Check your email')).toBeVisible();
	await page.goto(await signInLink(page.request, email));
	await page.waitForURL(/\/(signup|home|computers\/new)/);
	if (!page.url().includes('/signup')) return email;
	await page.getByLabel('Mobile number').fill(number);
	await page.getByRole('button', { name: 'Text me a code' }).click();
	await page.getByLabel('Code').fill(await smsCode(page.request, number));
	await page.getByRole('button', { name: 'Verify' }).click();
	await expect(page.getByRole('heading', { name: 'Choose a plan' })).toBeVisible();
	const plan = opts.plan ?? 'free';
	await page.locator(`[data-plan=${plan}]`).getByRole('button').click();
	if (plan !== 'free') {
		await page.getByLabel('Card number').fill(opts.card ?? '4242 4242 4242 4242');
		await page.getByLabel('Security code').fill('123');
		await page.getByRole('button', { name: /^Pay/ }).click();
	}
	await page.getByRole('button', { name: opts.trial ? 'Start the free trial' : /No thanks|Create your first computer/ }).click();
	await page.waitForURL(/computers\/new|home/);
	return email;
}

export async function newComputer(page: Page, name = 'Test box', size = 'small') {
	await page.goto('/computers/new');
	await page.getByLabel('Computer name').fill(name);
	await page.locator(`[data-size=${size}]`).click();
	await page.getByRole('button', { name: 'Create computer' }).click();
	await page.waitForURL(/\/home/);
	await expect(page.getByTestId('computer-switcher')).toContainText(name);
	await expect(page.getByTestId('computer-switcher')).toContainText(/Awake|Working/);
}

export async function api(page: Page, method: 'GET' | 'POST', path: string, data?: unknown) {
	const r = method === 'GET' ? await page.request.get(`/api${path}`) : await page.request.post(`/api${path}`, { data: data ?? {} });
	expect(r.ok(), `${method} ${path}: ${await r.text()}`).toBeTruthy();
	return r.json();
}
