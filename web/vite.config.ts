import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vitest/config';

// The browser only talks to this origin; /api goes to the control plane (the edge's job in production).
const api = process.env.API_URL ?? 'http://127.0.0.1:8080';
const proxy = {
	'/api': { target: api, changeOrigin: false },
	'/demo': { target: api, changeOrigin: false }
};

export default defineConfig({
	plugins: [sveltekit()],
	server: { port: Number(process.env.WEB_PORT ?? 5173), strictPort: true, proxy, fs: { allow: ['..'] } },
	preview: { port: Number(process.env.WEB_PORT ?? 5173), strictPort: true, proxy },
	test: { include: ['src/**/*.test.ts'] }
});
