export const sidebar = [
	{
		label: 'Get started',
		items: [
			{ slug: 'getting-started' },
			{ slug: 'getting-started/requirements' },
			{ slug: 'getting-started/install' },
			{ slug: 'getting-started/quickstart' },
		],
	},
	{
		label: 'Guides',
		items: [
			{ slug: 'guides/daily-workflow' },
			{ slug: 'guides/worktrees' },
			{ slug: 'guides/custom-image' },
			{ slug: 'guides/configuration-files' },
			{ slug: 'guides/teardown' },
		],
	},
	{
		label: 'Reference',
		items: [
			{ slug: 'reference/cli' },
			{ slug: 'reference/cli/add' },
			{ slug: 'reference/cli/apply' },
			{ slug: 'reference/cli/prepare' },
			{ slug: 'reference/cli/rebuild' },
			{ slug: 'reference/cli/open' },
			{ slug: 'reference/cli/stop' },
			{ slug: 'reference/cli/ls' },
			{ slug: 'reference/cli/status' },
			{ slug: 'reference/cli/destroy' },
			{ slug: 'reference/cli/global-options' },
			{ slug: 'reference/configuration' },
			{ slug: 'reference/filesystem' },
			{ slug: 'reference/output' },
		],
	},
	{
		label: 'Troubleshooting',
		items: [
			{ slug: 'troubleshooting' },
			{ slug: 'troubleshooting/host' },
			{ slug: 'troubleshooting/project' },
			{ slug: 'troubleshooting/safety-refusals' },
		],
	},
	{
		label: 'Project',
		items: [
			{ slug: 'project/design-principles' },
			{ slug: 'project/development' },
		],
	},
] as const;
