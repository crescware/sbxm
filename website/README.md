# sbxm website

The official sbxm documentation site is an Astro Starlight project with Tailwind CSS v4.
The website has its own Node and pnpm toolchain in `mise.toml`; it does not use the Rust toolchain
declared at the repository root.

## Local development

From this directory:

```sh
mise install
mise exec -- pnpm install --frozen-lockfile
mise exec -- pnpm dev
```

Open the URL printed by Astro. Search is available in a production build, so use the following when
you need to preview the final static output:

```sh
mise exec -- pnpm build
mise exec -- pnpm preview
```

The production build defaults to the GitHub Pages project path `/sbxm`. Use
`PUBLIC_SITE_URL` and `PUBLIC_BASE_PATH` when previewing another origin or base path.
