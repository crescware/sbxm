import { defineConfig } from 'astro/config';
import sitemap from '@astrojs/sitemap';
import starlight from '@astrojs/starlight';
import tailwindcss from '@tailwindcss/vite';
import { sidebar } from './src/config/sidebar.ts';

const site = process.env.PUBLIC_SITE_URL ?? 'https://crescware.github.io';
const base = process.env.PUBLIC_BASE_PATH || '/sbxm';
const publicBase = base === '/' ? '' : base;

export default defineConfig({
	site,
	base,
	integrations: [
		starlight({
			title: 'sbxm',
			description: 'A Docker Sandbox and predictable Git worktrees for every GitHub project.',
			locales: {
				root: { label: 'English', lang: 'en' },
			},
			social: [{ icon: 'github', label: 'GitHub', href: 'https://github.com/crescware/sbxm' }],
			sidebar,
			editLink: { baseUrl: 'https://github.com/crescware/sbxm/edit/main/website/' },
			lastUpdated: true,
			pagination: true,
			tableOfContents: { minHeadingLevel: 2, maxHeadingLevel: 3 },
			favicon: '/favicon.svg',
			customCss: ['./src/styles/global.css'],
			head: [
				{ tag: 'meta', attrs: { property: 'og:type', content: 'website' } },
				{ tag: 'meta', attrs: { property: 'og:site_name', content: 'sbxm' } },
				{ tag: 'meta', attrs: { property: 'og:image', content: `${site}${publicBase}/social-card.svg` } },
				{ tag: 'meta', attrs: { name: 'twitter:card', content: 'summary_large_image' } },
				{ tag: 'meta', attrs: { name: 'twitter:image', content: `${site}${publicBase}/social-card.svg` } },
			],
		}),
		sitemap(),
	],
	vite: {
		plugins: [tailwindcss()],
	},
});
