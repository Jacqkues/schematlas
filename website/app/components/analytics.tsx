"use client";

import { useEffect, useState } from "react";
import { CONSENT_KEY, parseAnalyticsConfig, privacySignal, readConsent, saveConsent, setAnalytics, track, type AnalyticsConfig, type Consent } from "@/lib/analytics";

const SETTINGS_EVENT = "schematlas:privacy-settings";

export function PrivacySettingsButton() {
  return <button className="privacy-settings" type="button" onClick={() => window.dispatchEvent(new Event(SETTINGS_EVENT))}>Privacy settings</button>;
}

export default function Analytics() {
  const [config, setConfig] = useState<AnalyticsConfig | null>(null);
  const [choice, setChoice] = useState<Consent | null>(null);
  const [open, setOpen] = useState(false);
  const [restricted, setRestricted] = useState(false);
  const [detailed, setDetailed] = useState(false);

  useEffect(() => {
    const controller = new AbortController();
    const show = () => { setDetailed(true); setOpen(true); };
    window.addEventListener(SETTINGS_EVENT, show);
    fetch("/api/analytics-config", { signal: controller.signal })
      .then(response => response.ok ? response.json() : { enabled: false })
      .then(value => {
        const settings = parseAnalyticsConfig(value);
        if (controller.signal.aborted) return;
        const restricted = privacySignal();
        const saved = restricted ? "essential" : readConsent();
        setRestricted(restricted);
        setConfig(settings);
        setChoice(saved);
        if (settings.enabled && !saved) setOpen(true);
        void setAnalytics(settings, saved ?? "essential");
      }).catch(() => { if (!controller.signal.aborted) setConfig({ enabled: false }); });
    return () => {
      controller.abort();
      window.removeEventListener(SETTINGS_EVENT, show);
    };
  }, []);

  useEffect(() => {
    if (!config) return;
    const sync = (event: StorageEvent) => {
      if (event.key !== CONSENT_KEY && event.key !== null) return;
      const saved = privacySignal() ? "essential" : readConsent();
      setChoice(saved);
      if (!saved) setDetailed(false);
      setOpen(config.enabled && !saved);
      void setAnalytics(config, saved ?? "essential");
    };
    window.addEventListener("storage", sync);
    return () => window.removeEventListener("storage", sync);
  }, [config]);

  useEffect(() => {
    if (!config?.enabled || !choice || choice === "essential") return;
    const click = (event: MouseEvent) => {
      const target = event.target as Element | null;
      const anchor = target?.closest("a");
      if (!anchor) return;
      const url = new URL(anchor.href);
      const location = anchor.closest("section")?.id || (anchor.closest(".hero") ? "hero" : anchor.closest("footer") ? "footer" : "page");
      if (url.hostname === "github.com" && url.pathname.startsWith("/Jacqkues/schematlas/")) {
        const download = url.pathname.match(/\/releases\/download\/(v[\d.]+)\/Schematlas-v[\d.]+-(macos|windows|linux)-(arm64|x64)\.(dmg|exe|msi|AppImage|deb)$/);
        if (download) {
          track("download_clicked", { release: download[1], platform: download[2], architecture: download[3], format: download[4], location });
          return;
        }
      }
      if (url.hostname === "github.com" && url.pathname.startsWith("/Jacqkues/schematlas")) {
        track("github_clicked", { location, destination: url.pathname.includes("/releases/") ? "releases" : url.pathname.includes("/issues") ? "issues" : url.pathname.includes("/blob/") ? "documentation" : "repository" });
      } else if (url.origin === window.location.origin && url.hash === "#download") {
        track("download_cta_clicked", { location });
      } else if (url.origin === window.location.origin && url.pathname === "/demo/index.html") {
        track("demo_opened_fullscreen");
      }
    };
    document.addEventListener("click", click, true);
    const seen = new Set<string>();
    const observer = new IntersectionObserver(entries => {
      for (const entry of entries) {
        if (!entry.isIntersecting) continue;
        const section = entry.target.id;
        if (seen.has(section)) continue;
        seen.add(section);
        track("section_viewed", { section });
      }
    }, { threshold: 0.35 });
    for (const id of ["workspace", "explore", "download"]) {
      const section = document.getElementById(id);
      if (section) observer.observe(section);
    }
    return () => {
      document.removeEventListener("click", click, true);
      observer.disconnect();
    };
  }, [config, choice]);

  function choose(next: Consent) {
    saveConsent(next);
    setChoice(next);
    setOpen(false);
    void setAnalytics(config ?? { enabled: false }, next);
  }

  function resetChoice() {
    void setAnalytics(config ?? { enabled: false }, "essential");
    try { localStorage.removeItem(CONSENT_KEY); } catch { /* Reset in memory if storage is unavailable. */ }
    setChoice(null);
    setDetailed(false);
    setOpen(true);
  }

  if (!open) return null;
  if (config?.enabled && !restricted && !detailed) {
    return <section className="analytics-preferences analytics-compact ph-no-capture" aria-label="Privacy choices">
      <p>We use analytics and session recordings to improve this site. Inputs are masked.</p>
      <div className="privacy-compact-actions">
        <button type="button" className="privacy-details-link" onClick={() => setDetailed(true)}>Settings</button>
        <button type="button" onClick={() => choose("essential")}>Reject</button>
        <button type="button" onClick={() => choose("replay")}>Accept</button>
      </div>
    </section>;
  }
  return <section className="analytics-preferences ph-no-capture" aria-labelledby="privacy-title" aria-live="polite">
    <div className="privacy-heading">
      <h2 id="privacy-title">Help make Schematlas better.</h2>
      {choice && <button type="button" className="privacy-close" aria-label="Close privacy settings" onClick={() => setOpen(false)}>×</button>}
    </div>
    {!config ? <p>Loading privacy settings…</p> : !config.enabled ? <>
      <p>Optional analytics and session recordings are not active on this site.</p>
      <button type="button" onClick={() => setOpen(false)}>Close</button>
    </> : restricted ? <>
      <p>Your browser’s privacy preference is enabled. Optional analytics and session recordings are off.</p>
      <button type="button" onClick={() => setOpen(false)}>Close</button>
    </> : <>
      <p>With your permission, PostHog measures visits and download clicks. You can also share a session recording of your clicks, scrolling, and demo interactions to help improve this page.</p>
      <details>
        <summary>What is collected?</summary>
        <p>Analytics uses a random browser identifier, device information, referring website, and page interactions. Session recordings reconstruct this page and the sample demo; they do not record your camera or screen outside this site. All input values are masked. Query strings, console logs, and network request content are excluded.</p>
        <p>Data is processed by PostHog in {config.region === "eu" ? "the EU" : "the US"}. Your choice is stored in this browser for 180 days. Change it anytime using Privacy settings in the footer. Withdrawing stops future collection; it does not delete data already collected.</p>
        <a href="https://posthog.com/privacy" target="_blank" rel="noopener noreferrer">PostHog privacy policy ↗</a>
        {choice && <button type="button" className="privacy-details-link" onClick={resetChoice}>Reset saved choice</button>}
      </details>
      <div className="privacy-choices">
        <button type="button" onClick={() => choose("essential")}>Reject optional</button>
        <button type="button" onClick={() => choose("analytics")}>Analytics only</button>
        <button type="button" onClick={() => choose("replay")}>Analytics + recordings</button>
      </div>
    </>}
  </section>;
}
