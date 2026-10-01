import { defineConfig, devices } from '@playwright/test';

// The browser tests run the whole stack through the one command, on its own ports with a
// fresh database (scripts/dev.sh --e2e).
export default defineConfig({
	testDir: './tests',
	timeout: 120_000,
	expect: { timeout: 20_000 },
	fullyParallel: false,
	workers: 1,
	reporter: process.env.CI ? [['list'], ['html', { open: 'never' }]] : 'list',
	use: {
		baseURL: 'http://localhost:15173',
		trace: 'retain-on-failure',
		screenshot: 'only-on-failure',
		...devices['Desktop Chrome'],
		viewport: { width: 1360, height: 900 }
	},
	webServer: {
		// E2E_DOCKER=1 runs the same flows with computers as Docker containers.
		command: `../scripts/dev.sh --e2e${process.env.E2E_DOCKER ? ' --docker' : ''}`,
		url: 'http://localhost:15173',
		timeout: 400_000,
		reuseExistingServer: !process.env.CI,
		stdout: 'pipe',
		// Let dev.sh stop its computers (agent processes or containers) on the way out.
		gracefulShutdown: { signal: 'SIGTERM', timeout: 15_000 }
	}
});
