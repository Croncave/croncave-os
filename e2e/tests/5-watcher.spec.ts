import { expect, test } from '@playwright/test';
import { api, newComputer, signUp } from './helpers';

test('set up a watch from a type, test it, save it, and see a planted listing found', async ({ page }) => {
	await signUp(page);
	await newComputer(page);
	await page.goto('/watcher');
	await page.getByRole('link', { name: 'New watch' }).click();
	await page.locator('[data-type=demo-listings]').click();
	await page.getByLabel('Most rent per month').fill('2200');
	await page.getByLabel('Listings page').fill('http://127.0.0.1:18080/demo/listings');
	await expect(page.getByTestId('plain-words')).toContainText('tell me about new listings up to $2200 a month with 1+ bedrooms');
	await expect(page.getByTestId('plain-words')).toContainText('every hour');

	// A test checks for real, saves nothing and tells no one.
	await page.getByRole('button', { name: 'Test it now' }).click();
	// Other tests plant listings too, so count loosely; the seeded one-bedroom always matches.
	await expect(page.getByTestId('test-result')).toContainText(/\d+ match(es)? on the first check/);
	await expect(page.getByTestId('match').filter({ hasText: 'Quiet one-bedroom' })).toBeVisible();

	await page.getByRole('button', { name: 'Save and start watching' }).click();
	await expect(page).toHaveURL(/\/watcher\/[0-9a-f-]{36}/);
	await expect(page.getByTestId('rule')).toContainText('every hour');
	await expect(page.getByText(/match(es)? on the first check/).first()).toBeVisible();

	// Plant a listing that fits on the demo site, then check now.
	await api(page, 'POST', '/dev/demo/listing', { title: 'Planted loft', price: 1500, beds: 2, area: 'Noe Valley' });
	await page.getByRole('button', { name: 'Check now' }).click();
	await expect(page.getByTestId('headline')).toHaveText('1 new match');
	await expect(page.getByTestId('match')).toContainText('Planted loft');
	await expect(page.getByTestId('unread')).toBeVisible();
});

test('a stock price watch draws its history and a missing page is "couldn\'t check"', async ({ page }) => {
	await signUp(page);
	await newComputer(page);
	await page.goto('/watcher/new');
	await page.locator('[data-type=stock-price]').click();
	await page.getByLabel('Price', { exact: true }).fill('5000');
	await page.getByRole('button', { name: 'Save and start watching' }).click();
	await expect(page.getByText(/ACME is \d+\.\d\d, below your 5000\.00/).first()).toBeVisible();
	await expect(page.getByRole('img', { name: 'Chart' }).first()).toBeVisible();

	await page.goto('/watcher/new');
	await page.locator('[data-type=web-page]').click();
	await page.getByLabel('Page address').fill('http://127.0.0.1:18080/demo/moved');
	await page.getByRole('button', { name: 'Test it now' }).click();
	await expect(page.getByTestId('test-result')).toContainText("Couldn't check");
	await expect(page.getByTestId('why')).toContainText('404');
});

test('the assistant is off until turned on, then proposes a watch you apply', async ({ page }) => {
	await signUp(page);
	await newComputer(page);
	await expect(page.getByRole('button', { name: 'Ask the assistant' })).toHaveCount(0);
	await page.goto('/settings');
	await page.getByTestId('ai-toggle').check();
	await expect(page.getByRole('button', { name: 'Ask the assistant' })).toBeVisible();
	await page.getByRole('button', { name: 'Ask the assistant' }).click();
	await page.getByRole('textbox', { name: 'Ask the assistant' }).fill('tell me when ACME stock goes below $90');
	await page.getByRole('button', { name: 'Ask', exact: true }).click();
	await expect(page.getByText('Watch ACME below $90')).toBeVisible();
	await page.getByRole('button', { name: 'Apply' }).click();
	await expect(page).toHaveURL(/\/watcher\//);
	await page.goto('/settings');
	await expect(page.getByText('assistant').first()).toBeVisible();
});
