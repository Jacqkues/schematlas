import type { Metadata } from 'next';
import { notFound } from 'next/navigation';
import Doc from '../doc';
import { find, pages } from '../pages';

type Props = { params: Promise<{ slug: string }> };

export function generateStaticParams() {
  return pages.filter((p) => p.slug !== 'introduction').map((p) => ({ slug: p.slug }));
}

export async function generateMetadata({ params }: Props): Promise<Metadata> {
  const page = find((await params).slug);
  return page ? { title: `${page.title} — Schematlas docs`, description: page.description } : {};
}

export default async function DocPage({ params }: Props) {
  const page = find((await params).slug);
  if (!page || page.slug === 'introduction') notFound();
  return <Doc page={page} />;
}
