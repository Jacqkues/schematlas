import type { Metadata } from 'next';
import './globals.css';
import './demo.css';
import './analytics.css';
import './repository.css';
import './editorial.css';
import './minimal.css';
import Analytics from './components/analytics';
import { ReleaseProvider } from './components/releases';
export const metadata: Metadata = {
  title: 'Schematlas — Understand your codebase',
  description:
    'Turn your repository into architecture maps, flows, and living documentation with your coding agent. Local desktop app. Free and open source.',
  icons: { icon: '/favicon.svg', shortcut: '/favicon.svg' },
  openGraph: {
    title: 'Schematlas — Understand your codebase',
    description:
      'Architecture maps, code links, and agent-written docs that live with your code. Free and open source for macOS, Windows, and Linux.',
    type: 'website',
  },
};
export default function RootLayout({ children }: Readonly<{ children: React.ReactNode }>) {
  return (
    <html lang="en">
      <head>
        <meta name="color-scheme" content="light" />
      </head>
      <body>
        <ReleaseProvider>{children}</ReleaseProvider>
        <Analytics />
      </body>
    </html>
  );
}
