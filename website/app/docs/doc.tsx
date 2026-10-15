import Link from 'next/link';
import { ArrowLeft, ArrowRight } from 'lucide-react';
import { Markdown, headings } from './markdown';
import { href, pages, type DocPage } from './pages';

const repo = 'https://github.com/Jacqkues/schematlas';

/** One documentation page: its text, the headings on it, and the pages around it. */
export default function Doc({ page }: { page: DocPage }) {
  const index = pages.indexOf(page);
  const previous = pages[index - 1];
  const next = pages[index + 1];
  const outline = headings(page.body);
  return (
    <>
      <article id="doc" className="d-article">
        <header>
          <h1>{page.title}</h1>
          {page.description && <p className="d-lede">{page.description}</p>}
        </header>
        <Markdown body={page.body} />
        <footer className="d-pager">
          {previous ? (
            <Link href={href(previous.slug)}>
              <span>
                <ArrowLeft size={13} /> Previous
              </span>
              {previous.title}
            </Link>
          ) : (
            <i />
          )}
          {next && (
            <Link href={href(next.slug)} className="d-next">
              <span>
                Next <ArrowRight size={13} />
              </span>
              {next.title}
            </Link>
          )}
        </footer>
        <p className="d-edit">
          <a href={`${repo}/edit/main/website/content/docs/${page.slug}.md`}>Edit this page</a>
        </p>
      </article>
      {outline.length > 1 && (
        <aside className="d-outline" aria-label="On this page">
          <p>On this page</p>
          <ul>
            {outline.map((h) => (
              <li key={h.id} data-level={h.level}>
                <a href={`#${h.id}`}>{h.text}</a>
              </li>
            ))}
          </ul>
        </aside>
      )}
    </>
  );
}
