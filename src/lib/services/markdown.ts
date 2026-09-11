import { Marked } from 'marked';
import DOMPurify from 'dompurify';

const parser = new Marked({ gfm: true, breaks: false, renderer: { html: () => '' } });

/** Agent output is untrusted. Allow document formatting, never executable HTML or remote images. */
export function renderMarkdown(text: string): string {
  if (typeof window === 'undefined') return '';
  const fragment = DOMPurify.sanitize(parser.parse(text, { async: false }), {
    ALLOWED_TAGS: [
      'p',
      'br',
      'strong',
      'em',
      'del',
      'blockquote',
      'pre',
      'code',
      'ul',
      'ol',
      'li',
      'h1',
      'h2',
      'h3',
      'h4',
      'h5',
      'h6',
      'hr',
      'table',
      'thead',
      'tbody',
      'tr',
      'th',
      'td',
      'a',
    ],
    ALLOWED_ATTR: ['href', 'title', 'start'],
    ALLOWED_URI_REGEXP: /^(?:(?:https?|mailto):|#)/i,
    RETURN_DOM_FRAGMENT: true,
  });
  for (const link of fragment.querySelectorAll('a[href]')) {
    link.setAttribute('target', '_blank');
    link.setAttribute('rel', 'noopener noreferrer');
  }
  const container = document.createElement('div');
  container.append(fragment);
  return container.innerHTML;
}
