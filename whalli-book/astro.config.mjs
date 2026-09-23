// @ts-check
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import fs from 'node:fs';

const whalliGrammar = JSON.parse(
	fs.readFileSync(new URL('./whalli.tmLanguage.json', import.meta.url), 'utf-8')
);

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
			customCss: [
				'@fontsource/inter/400.css',
				'@fontsource/inter/600.css',
				'@fontsource/jetbrains-mono/400.css',
				'@fontsource/jetbrains-mono/600.css',
				'./src/styles/custom.css',
			],
			expressiveCode: {
				shiki: {
					langs: [whalliGrammar],
				},
				themes: ['github-dark', 'github-light'],
			},
			sidebar: [
				{
					label: 'The Whalli Book',
					items: [
						{ label: '1. Introduction', slug: 'intro' },
						{ label: '2. Quick Start', slug: 'quick-start' },
						{ label: '3. Language Tour', slug: 'language-tour' },
						{ label: '4. Concurrency & Woroutines', slug: 'concurrency' },
						{ label: '5. I/O and HTTP Backend', slug: 'io-net' },
						{ label: '6. Standard Library & SQL', slug: 'stdlib' },
						{ label: '7. Toolchain and VM', slug: 'toolchain' },
						{ label: '8. Tooling & IDE Support', slug: 'tooling' },
						{ label: '9. Complete Examples', slug: 'examples' },
					],
				},
			],
			head: [
				{
					tag: 'meta',
					attrs: { property: 'og:image', content: 'https://whalli.is-a.dev/whalli.png' },
				},
				{
					tag: 'meta',
					attrs: { name: 'twitter:card', content: 'summary_large_image' },
				},
			],
		}),
	],
});
