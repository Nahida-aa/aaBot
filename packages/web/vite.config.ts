import { paraglideVitePlugin } from '@inlang/paraglide-js';
import tailwindcss from '@tailwindcss/vite';
import { devtools } from '@tanstack/devtools-vite';
import { tanstackRouter } from '@tanstack/router-plugin/vite';
import { defineConfig } from 'vite';
import solid from 'vite-plugin-solid';
// bun add -D @inlang/paraglide-js -D @tailwindcss/vite -D @tanstack/devtools-vite -D @tanstack/router-plugin -D @typescript/native-preview -D vite

export default defineConfig(async () => ({
  server: {
    port: 5173,
    proxy: {
      "/api": {
        target: "http://localhost:3000",
        changeOrigin: true,
        rewrite: (path: string) => path.replace(/^\/api/, ""),
      },
    },
  },
  plugins: [		
    devtools(),
  	paraglideVitePlugin({
			project: '../../packages/shared/i18n/project.inlang',
			outdir: '../../packages/shared/i18n/paraglide',
			strategy: ['cookie', 'preferredLanguage', 'baseLocale'],
		}),
    tailwindcss(),
		tanstackRouter({ target: 'solid', autoCodeSplitting: true }),
		solid(),
  ],
}));
