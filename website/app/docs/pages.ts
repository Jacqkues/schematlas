import introduction from '../../content/docs/introduction.md?raw';
import howItWorks from '../../content/docs/how-it-works.md?raw';
import formats from '../../content/docs/formats.md?raw';
import wiki from '../../content/docs/wiki.md?raw';
import codeChanges from '../../content/docs/code-changes.md?raw';
import agents from '../../content/docs/agents.md?raw';
import databases from '../../content/docs/databases.md?raw';
import syncAndPrivacy from '../../content/docs/sync-and-privacy.md?raw';
import { parse } from './markdown';

// The documentation, in reading order. Pages are Markdown files in content/docs.
const outline: { title: string; pages: [string, string][] }[] = [
  { title: 'Get started', pages: [['introduction', introduction]] },
  {
    title: 'Guides',
    pages: [
      ['how-it-works', howItWorks],
      ['formats', formats],
      ['wiki', wiki],
      ['code-changes', codeChanges],
      ['agents', agents],
      ['databases', databases],
    ],
  },
  { title: 'Reference', pages: [['sync-and-privacy', syncAndPrivacy]] },
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
