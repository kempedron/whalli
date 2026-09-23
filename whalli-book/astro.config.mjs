// @ts-check
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

// https://astro.build/config
export default defineConfig({
	site: 'https://whalli.is-a.dev',
	base: '/',
	integrations: [
		starlight({
			title: 'Whalli',
			description: 'Lightweight modern language with woroutines, Keep-Alive HTTP & SQL — learn Whalli from scratch.',
			logo: {
				src: './src/assets/whalli.png',
			},
			favicon: '/favicon.svg',
			social: [{ icon: 'github', label: 'GitHub', href: 'https://github.com/kempedron/whalli' }],
			editLink: {
				baseUrl: 'https://github.com/kempedron/whalli/edit/main/whalli-book/',
			},
			customCss: ['./src/styles/custom.css'],
			sidebar: [
				{
					label: 'Book',
					items: [
						{ label: 'Introduction', slug: 'intro' },
						{ label: 'Quick Start', slug: 'quick-start' },
						{ label: 'Language Tour', slug: 'language-tour' },
						{ label: 'Concurrency', slug: 'concurrency' },
						{ label: 'I/O and Networking', slug: 'io-net' },
						{ label: 'Standard Library', slug: 'stdlib' },
						{ label: 'Toolchain', slug: 'toolchain' },
						{ label: 'Tooling', slug: 'tooling' },
						{ label: 'Examples', slug: 'examples' },
					],
				},
			],
			head: [
				{
					tag: 'meta',
					attrs: { property: 'og:image', content: 'https://whalli.is-a.dev/og.png' },
				},
			],
		}),
	],
});
