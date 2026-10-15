import introduction from '../../content/docs/introduction.md?raw';
import gettingStarted from '../../content/docs/getting-started.md?raw';
import howItWorks from '../../content/docs/how-it-works.md?raw';
import toc from '../../content/docs/table-of-contents.md?raw';
import mermaid from '../../content/docs/mermaid.md?raw';
import drawio from '../../content/docs/drawio.md?raw';
import dbml from '../../content/docs/dbml.md?raw';
import wiki from '../../content/docs/wiki.md?raw';
import annotations from '../../content/docs/annotations.md?raw';
import codeChanges from '../../content/docs/code-changes.md?raw';
import agents from '../../content/docs/agents.md?raw';
import databases from '../../content/docs/databases.md?raw';
import teamSync from '../../content/docs/team-sync.md?raw';
import privacy from '../../content/docs/privacy.md?raw';
import { parse } from './markdown';

// The documentation, in reading order. Pages are Markdown files in content/docs.
const outline: { title: string; pages: [string, string][] }[] = [
  {
    title: 'Get started',
    pages: [
      ['introduction', introduction],
      ['getting-started', gettingStarted],
    ],
  },
  {
    title: 'Concepts',
    pages: [
      ['how-it-works', howItWorks],
      ['table-of-contents', toc],
    ],
  },
  {
    title: 'Formats',
    pages: [
      ['mermaid', mermaid],
      ['drawio', drawio],
      ['dbml', dbml],
      ['wiki', wiki],
      ['annotations', annotations],
    ],
  },
  {
    title: 'Features',
    pages: [
      ['code-changes', codeChanges],
      ['agents', agents],
      ['databases', databases],
      ['team-sync', teamSync],
    ],
  },
  { title: 'Reference', pages: [['privacy', privacy]] },
];

export type DocPage = { slug: string; title: string; description: string; body: string };

export const sections = outline.map((section) => ({
  title: section.title,
  pages: section.pages.map(([slug, source]): DocPage => {
    const { meta, body } = parse(source);
    return { slug, title: meta.title ?? slug, description: meta.description ?? '', body };
  }),
}));

export const pages = sections.flatMap((section) => section.pages);

export function find(slug: string) {
  return pages.find((page) => page.slug === slug);
}

export function href(slug: string) {
  return slug === 'introduction' ? '/docs' : `/docs/${slug}`;
}
