import { paraglideVitePlugin } from '@inlang/paraglide-js';
import tailwindcss from '@tailwindcss/vite';
import { devtools } from '@tanstack/devtools-vite';
import { tanstackRouter } from '@tanstack/router-plugin/vite';
import { defineConfig } from 'vite';
import solid from 'vite-plugin-solid';
// bun add -D @inlang/paraglide-js -D @tailwindcss/vite -D @tanstack/devtools-vite -D @tanstack/router-plugin -D @typescript/native-preview -D vite

export default defineConfig(async () => ({
  plugins: [		
    devtools(),
    tailwindcss(),
		tanstackRouter({ target: 'solid', autoCodeSplitting: true }),
		solid(),
  ],
}));
