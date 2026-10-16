'use client';

import { usePathname } from 'next/navigation';

export type NavSection = { title: string; pages: { href: string; title: string }[] };

/** The section list, with the page being read marked. */
export default function DocsNav({ sections }: { sections: NavSection[] }) {
  const path = usePathname()?.replace(/\/$/, '') || '/docs';
  return (
    <nav className="d-nav" aria-label="Documentation">
      {sections.map((section) => (
        <div key={section.title}>
          <p>{section.title}</p>
          <ul>
            {section.pages.map((page) => (
              <li key={page.href}>
                <a href={page.href} aria-current={path === page.href ? 'page' : undefined}>
                  {page.title}
                </a>
              </li>
            ))}
          </ul>
        </div>
      ))}
    </nav>
  );
}
