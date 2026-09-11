// @vitest-environment jsdom
import { describe, expect, it } from 'vitest';
import { renderMarkdown } from './markdown';

function render(text: string) {
  const root = document.createElement('div');
  root.innerHTML = renderMarkdown(text);
  return root;
}

describe('agent Markdown', () => {
  it('renders headings, emphasis, nested lists, tables, and fenced SQL', () => {
    const root = render(
      '# Domains\n\n**Auth** and *Users*\n\n- Accounts\n  - Sessions\n\n| Domain | Tables |\n| --- | --- |\n| Auth | 13 |\n\n```sql\nSELECT 1 < 2;\n```',
    );
    expect(root.querySelector('h1')?.textContent).toBe('Domains');
    expect(root.querySelector('strong')?.textContent).toBe('Auth');
    expect(root.querySelector('ul ul li')?.textContent).toBe('Sessions');
    expect(root.querySelector('td')?.textContent).toBe('Auth');
    expect(root.querySelector('pre code')?.textContent).toBe('SELECT 1 < 2;\n');
  });
  it('removes executable markup, unsafe links, and remote images', () => {
    const root = render(
      '<script>alert(1)</script>\n\n<img src=x onerror=alert(1)>\n\n[bad](javascript:alert%281%29) ![remote](https://example.com/tracker)\n\n[encoded](java&#x73;cript:alert%281%29) [file](file:///etc/passwd)\n\n[docs](https://example.com/docs)',
    );
    expect(root.querySelector('script,img,iframe,svg,style,[onerror]')).toBeNull();
    expect([...root.querySelectorAll('a[href]')].map((a) => a.getAttribute('href'))).toEqual([
      'https://example.com/docs',
    ]);
    const link = root.querySelector('a[href]');
    expect(link?.getAttribute('rel')).toBe('noopener noreferrer');
    expect(link?.getAttribute('target')).toBe('_blank');
  });
  it('handles incomplete streamed Markdown and escapes HTML within code', () => {
    expect(render('**still streaming').textContent).toContain('still streaming');
    const root = render('```html\n<img src=x onerror=alert(1)>');
    expect(root.querySelector('img')).toBeNull();
    expect(root.querySelector('code')?.textContent).toContain('<img src=x onerror=alert(1)>');
  });
});
