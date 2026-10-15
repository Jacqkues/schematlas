import { Fragment, type ReactNode } from 'react';

// A small Markdown renderer for the documentation pages, which we write ourselves: headings,
// paragraphs, lists, tables, fenced code, block quotes, and inline code, links, bold and
// italic. Text is rendered as React text, never as HTML.

export type Heading = { id: string; text: string; level: number };

export function slug(text: string) {
  return text
    .toLowerCase()
    .replace(/`/g, '')
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-|-$/g, '');
}

function inline(text: string, key = 'i'): ReactNode[] {
  const out: ReactNode[] = [];
  const pattern = /(`[^`]+`)|(\*\*[^*]+\*\*)|(\*[^*\s][^*]*\*)|(\[[^\]]+\]\([^)\s]+\))/g;
  let last = 0;
  let n = 0;
  for (const match of text.matchAll(pattern)) {
    if (match.index! > last) out.push(text.slice(last, match.index));
    const token = match[0];
    const k = `${key}-${n++}`;
    if (token.startsWith('`')) out.push(<code key={k}>{token.slice(1, -1)}</code>);
    else if (token.startsWith('**'))
      out.push(<strong key={k}>{inline(token.slice(2, -2), k)}</strong>);
    else if (token.startsWith('*')) out.push(<em key={k}>{inline(token.slice(1, -1), k)}</em>);
    else {
      const [, label, href] = token.match(/^\[([^\]]+)\]\(([^)\s]+)\)$/)!;
      const external = /^https?:/.test(href);
      out.push(
        <a
          key={k}
          href={href}
          {...(external ? { target: '_blank', rel: 'noopener noreferrer' } : {})}
        >
          {inline(label, k)}
        </a>,
      );
    }
    last = match.index! + token.length;
  }
  if (last < text.length) out.push(text.slice(last));
  return out;
}

function cells(row: string) {
  return row
    .trim()
    .replace(/^\||\|$/g, '')
    .split(/(?<!\\)\|/)
    .map((c) => c.trim().replace(/\\\|/g, '|'));
}

/** Front matter (`title`, `description`) and the body of a page. */
export function parse(source: string) {
  const meta: Record<string, string> = {};
  let body = source;
  const front = source.match(/^---\n([\s\S]*?)\n---\n/);
  if (front) {
    for (const line of front[1].split('\n')) {
      const [key, ...rest] = line.split(':');
      if (key && rest.length) meta[key.trim()] = rest.join(':').trim().replace(/^"|"$/g, '');
    }
    body = source.slice(front[0].length);
  }
  return { meta, body };
}

export function headings(body: string): Heading[] {
  const out: Heading[] = [];
  let fenced = false;
  for (const line of body.split('\n')) {
    if (line.startsWith('```')) fenced = !fenced;
    const m = !fenced && line.match(/^(#{2,3})\s+(.*)$/);
    if (m) out.push({ id: slug(m[2]), text: m[2].replace(/`/g, ''), level: m[1].length });
  }
  return out;
}

export function Markdown({ body }: { body: string }) {
  const lines = body.split('\n');
  const blocks: ReactNode[] = [];
  let i = 0;
  const key = () => `b${blocks.length}`;
  while (i < lines.length) {
    const line = lines[i];
    if (!line.trim()) {
      i++;
      continue;
    }
    if (line.startsWith('```')) {
      const lang = line.slice(3).trim();
      const code: string[] = [];
      i++;
      while (i < lines.length && !lines[i].startsWith('```')) code.push(lines[i++]);
      i++;
      blocks.push(
        <figure className="d-code" key={key()}>
          {lang && <figcaption>{lang}</figcaption>}
          <pre>
            <code>{code.join('\n')}</code>
          </pre>
        </figure>,
      );
      continue;
    }
    const heading = line.match(/^(#{2,4})\s+(.*)$/);
    if (heading) {
      const level = heading[1].length;
      const Tag = `h${level}` as 'h2' | 'h3' | 'h4';
      blocks.push(
        <Tag key={key()} id={slug(heading[2])}>
          {inline(heading[2])}
        </Tag>,
      );
      i++;
      continue;
    }
    if (line.startsWith('|')) {
      const rows: string[] = [];
      while (i < lines.length && lines[i].startsWith('|')) rows.push(lines[i++]);
      const [head, , ...rest] = rows;
      blocks.push(
        <div className="d-table" key={key()}>
          <table>
            <thead>
              <tr>
                {cells(head).map((c, j) => (
                  <th key={j}>{inline(c)}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {rest.map((row, r) => (
                <tr key={r}>
                  {cells(row).map((c, j) => (
                    <td key={j}>{inline(c)}</td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>,
      );
      continue;
    }
    if (line.startsWith('> ')) {
      const quote: string[] = [];
      while (i < lines.length && lines[i].startsWith('>')) quote.push(lines[i++].slice(1).trim());
      blocks.push(
        <aside className="d-note" key={key()}>
          {inline(quote.join(' '))}
        </aside>,
      );
      continue;
    }
    const list = line.match(/^(\s*)([-*]|\d+\.)\s+/);
    if (list) {
      const ordered = /\d/.test(list[2]);
      const items: string[] = [];
      while (i < lines.length && /^(\s*)([-*]|\d+\.)\s+/.test(lines[i])) {
        let item = lines[i++].replace(/^(\s*)([-*]|\d+\.)\s+/, '');
        while (
          i < lines.length &&
          /^\s{2,}\S/.test(lines[i]) &&
          !/^\s*([-*]|\d+\.)\s/.test(lines[i])
        )
          item += ' ' + lines[i++].trim();
        items.push(item);
      }
      const List = ordered ? 'ol' : 'ul';
      blocks.push(
        <List key={key()}>
          {items.map((item, j) => (
            <li key={j}>{inline(item)}</li>
          ))}
        </List>,
      );
      continue;
    }
    const paragraph: string[] = [];
    while (
      i < lines.length &&
      lines[i].trim() &&
      !/^(```|#{2,4}\s|\||> |\s*([-*]|\d+\.)\s)/.test(lines[i])
    )
      paragraph.push(lines[i++].trim());
    blocks.push(<p key={key()}>{inline(paragraph.join(' '))}</p>);
  }
  return <Fragment>{blocks}</Fragment>;
}
