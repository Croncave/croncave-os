import { expect, test } from '@playwright/test';
import path from 'node:path';
import { newComputer, signUp } from './helpers';

const fixture = (f: string) => path.join(import.meta.dirname, '..', 'fixtures', f);

test('upload, browse, preview a table, an image and text, delete to Trash and restore', async ({ page }) => {
	await signUp(page);
	await newComputer(page);
	await page.goto('/files');
	page.once('dialog', (d) => d.accept('Docs'));
	await page.getByRole('button', { name: 'New folder' }).click();
	await page.getByTestId('file-row').filter({ hasText: 'Docs' }).click();
	await expect(page).toHaveURL(/path=Docs/);

	await page.getByTestId('upload-input').setInputFiles([fixture('people.csv'), fixture('pattern.png'), fixture('notes.txt')]);
	await expect(page.getByTestId('file-row')).toHaveCount(3);
	await expect(page.getByTestId('file-row').filter({ hasText: 'people.csv' })).toContainText('Uploaded by you');

	await page.getByTestId('file-row').filter({ hasText: 'people.csv' }).click();
	await expect(page.getByTestId('preview-table')).toContainText('Grace');
	await expect(page.getByText('Showing 3 of 3 rows')).toBeVisible();
	await page.getByTestId('file-row').filter({ hasText: 'pattern.png' }).click();
	await expect(page.getByTestId('preview-image')).toBeVisible();
	await expect(page.getByText('64 × 48')).toBeVisible();
	await page.getByTestId('file-row').filter({ hasText: 'notes.txt' }).click();
	await expect(page.getByTestId('preview-text')).toContainText('water the plants');

	await page.getByRole('button', { name: 'Delete' }).click();
	await expect(page.getByTestId('file-row')).toHaveCount(2);
	await page.getByRole('button', { name: 'Trash' }).click();
	await expect(page.locator('table')).toContainText('Docs/notes.txt');
	await expect(page.locator('table')).toContainText('Deleted by you');
	await page.getByRole('button', { name: 'Restore' }).click();
	await page.getByRole('button', { name: 'Back to files' }).click();
	await expect(page.getByTestId('file-row')).toHaveCount(3);
});
