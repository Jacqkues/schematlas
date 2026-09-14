"use client";

import { createContext, useContext, useEffect, useState, type ReactNode } from "react";
import { ArrowUpRight } from "lucide-react";
import { FALLBACK_RELEASE, parseReleaseInfo, type DownloadKind, type ReleaseInfo } from "@/lib/releases";

const ReleaseContext = createContext<ReleaseInfo>(FALLBACK_RELEASE);

export function ReleaseProvider({ children }: { children: ReactNode }) {
  const [release, setRelease] = useState(FALLBACK_RELEASE);
  useEffect(() => {
    const controller = new AbortController();
    const timeout = window.setTimeout(() => controller.abort(), 6000);
    fetch("/api/releases/latest", { signal: controller.signal, priority: "low" })
      .then(response => response.ok ? response.json() : null)
      .then(value => { if (!controller.signal.aborted) setRelease(parseReleaseInfo(value) ?? FALLBACK_RELEASE); })
      .catch(() => { /* Server-rendered GitHub links remain usable. */ })
      .finally(() => window.clearTimeout(timeout));
    return () => { controller.abort(); window.clearTimeout(timeout); };
  }, []);
  return <ReleaseContext.Provider value={release}>{children}</ReleaseContext.Provider>;
}

export function LatestReleaseLink({ hero = false }: { hero?: boolean }) {
  const release = useContext(ReleaseContext);
  const label = hero
    ? release.tag ? `VERSION ${release.tag.replace(/^v/, "")}` : "LATEST RELEASE"
    : release.tag ? `Release notes · ${release.tag}` : "Release notes";
  return <a href={release.url} className={hero ? "hero-release" : undefined}>
    {hero && <span className="status-dot" />}{label}<ArrowUpRight size={hero ? 12 : 15} />
  </a>;
}

export function ReleaseDownload({ kind, children }: { kind: DownloadKind; children: ReactNode }) {
  const release = useContext(ReleaseContext);
  const url = release.assets[kind];
  return <a className="download-link" href={url ?? release.url}
    title={url ? undefined : "View available downloads on GitHub"}>{children}</a>;
}
