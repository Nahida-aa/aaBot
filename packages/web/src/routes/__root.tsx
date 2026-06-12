// import appCss from '@repo/shared/styles/index.css?url';

import { AppSidebar } from '#/components/app/AppSidebar.tsx';
import { Devtools } from '@repo/ui-solid/app/devtools';
import { SidebarProvider } from '@repo/ui-solid/base/sidebar';
import { Toaster } from '@repo/ui-solid/base/sonner';
import { ModalRenderer } from '@repo/ui-solid/custom/modal/renderer';
import { ThemeProvider, themeScript } from '@repo/ui-solid/theme';
import type { QueryClient } from '@tanstack/solid-query';
import {
	createRootRouteWithContext,
	HeadContent,
	Link,
	Outlet,
} from '@tanstack/solid-router';
import { type JSX, Suspense } from 'solid-js';
import {
	getLocale,
} from '@repo/shared/i18n/paraglide/runtime.js';

export const Route = createRootRouteWithContext<{
	queryClient: QueryClient;
}>()({
	head: () => ({
		// links: [
		// 	{
		// 		rel: 'stylesheet',
		// 		href: appCss,
		// 	},
		// ],
		scripts: [{ children: themeScript }],
	}),
  	beforeLoad: async ({ context: { queryClient } }) => {
		// Other redirect strategies are possible; see
		// https://github.com/TanStack/router/tree/main/examples/react/i18n-paraglide#offline-redirect
		if (typeof document !== 'undefined') {
			document.documentElement.setAttribute('lang', getLocale());
		}

	},
	component: RootComponent,
});

function RootComponent() {
	return (
		<>
			<HeadContent />
			<div class="antialiased min-h-dvh flex flex-col">
				<Suspense>
					<ThemeProvider>
						<SidebarProvider>
							<AppSidebar />
						<div class="w-full h-screen grid mx-auto  ">
							<header ></header>
							<main class="flex-1">
								<Outlet />
								<ModalRenderer />
							</main>
						</div>
						</SidebarProvider>
						<Toaster duration={1000 * 10} />
					</ThemeProvider>

					<Devtools />
				</Suspense>
			</div>
		</>
	);
}
