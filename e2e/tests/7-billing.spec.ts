import { expect, test } from '@playwright/test';
import { api, newComputer, signUp } from './helpers';

test('usage draws from the award, then the allowance; the cap pauses work; a paid plan lets it resume', async ({ page }) => {
	await signUp(page, { plan: 'free' });
	await newComputer(page);
	await page.goto('/plans');
	await expect(page.getByTestId('balances')).toContainText('$3.00');
	// Free can't opt in to overage.
	await expect(page.getByText('so a free account never becomes a bill')).toBeVisible();

	await api(page, 'POST', '/dev/usage', { dollars: 2.5 });
	await expect(page.getByTestId('balances')).toContainText('$0.50');
	await expect(page.getByTestId('balances')).not.toContainText('Sign-up award');
	await api(page, 'POST', '/dev/usage', { dollars: 1 });
	await expect(page.getByTestId('usage-ran-out')).toBeVisible();
	await page.goto('/home');
	await expect(page.getByTestId('paused-banner')).toBeVisible();
	const me = await api(page, 'GET', '/me');
	const run = await page.request.post(`/api/computers/${me.computers[0].id}/action`, { data: { action: 'wake' } });
	expect(run.status()).toBe(402);

	// Switch to Plus, turn on overage with a $10 cap: work resumes.
	await page.goto('/plans');
	await page.locator('[data-plan=plus]').getByRole('button', { name: 'Switch to Plus' }).click();
	await page.getByLabel('Card number').fill('4242 4242 4242 4242');
	await page.getByLabel('Security code').fill('123');
	await page.getByRole('button', { name: 'Confirm' }).click();
	await page.getByTestId('overage').check();
	await page.getByLabel('Cap in dollars').fill('10');
	await page.getByRole('button', { name: 'Save', exact: true }).click();
	await expect(page.getByTestId('usage-ran-out')).toHaveCount(0);
	await expect(page.locator('a[href^="/invoices/"]')).toHaveCount(1);
	await page.locator('a[href^="/invoices/"]').first().click();
	await expect(page.getByText('Plus plan from')).toBeVisible();
});

test('a trial ends with no charge', async ({ page }) => {
	await signUp(page, { plan: 'free', trial: true });
	await page.goto('/plans');
	await expect(page.getByTestId('trial-banner')).toContainText('Plus: 14 days left');
	await api(page, 'POST', '/dev/clock', { secs: 15 * 86400 });
	await expect(page.getByTestId('trial-banner')).toHaveCount(0);
	await page.getByTestId('bell').click();
	await expect(page.getByTestId('activity')).toContainText('Your Plus trial ended');
	await expect(page.getByTestId('activity')).toContainText('Nothing was charged.');
	await expect(page.getByText('No invoices yet.')).toBeVisible();
});
