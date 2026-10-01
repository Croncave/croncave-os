import { expect, test } from '@playwright/test';
import { phone, signInLink, smsCode, unique } from './helpers';

test('sign up with an email link and a phone code, pay for Plus, get the award, start the trial', async ({ page }) => {
	const email = unique('signup');
	const number = phone();
	await page.goto('/');
	await expect(page).toHaveURL(/signin/);
	await page.getByLabel('Email').fill(email);
	await page.getByRole('button', { name: 'Send sign-in link' }).click();
	await expect(page.getByRole('heading', { name: 'Check your email' })).toBeVisible();

	// The sign-in link arrives in the outbox (Dev tools).
	await page.getByTestId('outbox-link').click();
	await expect(page.getByTestId('outbox')).toContainText('Your Croncave sign-in link');
	await page.goto(await signInLink(page.request, email));

	await expect(page.getByRole('heading', { name: 'Your details and phone' })).toBeVisible();
	await page.getByLabel('Your name').fill('Maya Chen');
	await page.getByRole('button', { name: 'Time zone' }).click();
	await page.getByRole('option', { name: /Pacific/ }).click();
	await page.getByLabel('Mobile number').fill('+44 20 7946 0958');
	await page.getByRole('button', { name: 'Send code' }).click();
	await expect(page.getByText(/only available in the United States/)).toBeVisible();
	await page.getByLabel('Mobile number').fill(number);
	await page.getByRole('button', { name: 'Send code' }).click();
	await expect(page.getByRole('heading', { name: 'Confirm your phone' })).toBeVisible();
	// The code checks itself once all six digits are in.
	await page.getByLabel('Code', { exact: true }).fill('000000');
	await expect(page.getByText(/code isn't right/)).toBeVisible();
	await page.getByLabel('Code', { exact: true }).fill(await smsCode(page.request, number));

	await expect(page.getByRole('heading', { name: 'Choose a plan' })).toBeVisible();
	await expect(page.locator('[data-plan=plus]')).toContainText('$10');
	await page.locator('[data-plan=plus]').getByRole('button').click();
	// A declined test card, then a good one.
	await page.getByLabel('Card number').fill('4000 0000 0000 0002');
	await page.getByLabel('Security code').fill('123');
	await page.getByRole('button', { name: /^Pay \$10/ }).click();
	await expect(page.getByTestId('card-error')).toHaveText('Your card was declined.');
	await page.getByLabel('Card number').fill('4242 4242 4242 4242');
	await page.getByRole('button', { name: /^Pay \$10/ }).click();

	await expect(page.getByTestId('award')).toContainText('$5.00');
	await expect(page.getByRole('heading', { name: 'Try Pro free for 7 days' })).toBeVisible();
	await expect(page.getByText('Nothing is charged automatically.')).toBeVisible();
	await page.getByRole('button', { name: 'Start my Pro trial' }).click();
	await expect(page.getByRole('heading', { name: 'Set up your first computer' })).toBeVisible();

	// The name and time zone from sign-up are kept; new computers take the time zone.
	const me = await (await page.request.get('/api/me')).json();
	expect(me.user.name).toBe('Maya Chen');
	expect(me.user.time_zone).toBe('America/Los_Angeles');
	await page.locator('[data-size=small]').click();
	await expect(page).toHaveURL(/\/home/);
	const after = await (await page.request.get('/api/me')).json();
	expect(after.computers[0].time_zone).toBe('America/Los_Angeles');

	await page.goto('/plans');
	await expect(page.getByTestId('trial-banner')).toContainText("You're trying Pro: 7 days left.");
	await expect(page.getByTestId('balances')).toContainText('Sign-up award');
});
