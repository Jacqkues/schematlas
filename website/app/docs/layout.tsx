import type { Metadata } from 'next';
import '../docs.css';
import DocsNav from './nav';
import { href, sections } from './pages';

export const metadata: Metadata = {
  title: 'Documentation — Schematlas',
  description:
    'How Schematlas works: diagrams and docs as files in your repository, annotations in Mermaid, draw.io and DBML, agents, code-change flags and databases.',
};

const repo = 'https://github.com/Jacqkues/schematlas';

export default function DocsLayout({ children }: { children: React.ReactNode }) {
  return (
    <div className="d-shell">
      <a className="skip-link" href="#doc">
        Skip to content
      </a>
      <header className="m-nav d-header">
        <a href="/" className="m-brand" aria-label="Schematlas home">
          <img src="/favicon.svg" alt="" width={24} height={24} />
          <span>Schematlas</span>
          <em className="d-badge">Docs</em>
        </a>
        <nav aria-label="Main navigation">
          <a href="/docs">Documentation</a>
          <a href={repo}>GitHub</a>
          <a href="/#download" className="m-nav-download">
            Download
          </a>
        </nav>
      </header>
      <div className="d-body">
        <DocsNav
          sections={sections.map((s) => ({
            title: s.title,
            pages: s.pages.map((p) => ({ href: href(p.slug), title: p.title })),
          }))}
        />
        {children}
      </div>
    </div>
  );
}
