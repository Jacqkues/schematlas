import type { PostHog, CaptureResult } from "posthog-js";

export type Consent = "essential" | "analytics" | "replay";
export type AnalyticsConfig = { enabled: true; key: string; host: string; region: "eu" | "us" } | { enabled: false };
export function parseAnalyticsConfig(value: unknown): AnalyticsConfig {
  if (!value || typeof value !== "object") return { enabled: false };
  const item = value as Record<string, unknown>;
  if (item.enabled !== true || typeof item.key !== "string" || !/^phc_[a-zA-Z0-9_-]+$/.test(item.key)) return { enabled: false };
  if (item.region !== "eu" && item.region !== "us") return { enabled: false };
  const host = item.region === "eu" ? "https://eu.i.posthog.com" : "https://us.i.posthog.com";
  return item.host === host ? { enabled: true, key: item.key, host, region: item.region } : { enabled: false };
}
export const CONSENT_KEY = "schematlas.analytics-consent.v1";
const CONSENT_LIFETIME = 180 * 24 * 60 * 60 * 1000;
type Properties = Record<string, string | number | boolean>;
let client: PostHog | undefined;
let consent: Consent = "essential";
let generation = 0;
let pageviewSent = false;
let pending: { name: string; properties: Properties }[] = [];

export function privacySignal() {
  return navigator.doNotTrack === "1" || (navigator as Navigator & { globalPrivacyControl?: boolean }).globalPrivacyControl === true;
}

export function readConsent(): Consent | null {
  try {
    const saved = JSON.parse(localStorage.getItem(CONSENT_KEY) ?? "null");
    if (saved && ["essential", "analytics", "replay"].includes(saved.choice)
      && Number.isFinite(saved.at) && saved.at <= Date.now() && Date.now() - saved.at < CONSENT_LIFETIME) return saved.choice;
  } catch { /* Storage can be unavailable in private browsing. */ }
  return null;
}

export function saveConsent(choice: Consent) {
  try { localStorage.setItem(CONSENT_KEY, JSON.stringify({ choice, at: Date.now() })); } catch { /* Keep this choice in memory. */ }
}

function cleanUrl(value: string) {
  try { const url = new URL(value); return url.origin + url.pathname; } catch { return ""; }
}

function sanitize(event: CaptureResult | null): CaptureResult | null {
  if (!event || consent === "essential" || privacySignal()) return null;
  if (event.event === "$snapshot" && consent !== "replay") return null;
  for (const name of ["$current_url", "$initial_current_url", "$session_entry_url"]) {
    if (typeof event.properties[name] === "string") event.properties[name] = cleanUrl(event.properties[name]);
  }
  // Referrer origin is useful for attribution; paths and search parameters are unnecessary.
  for (const name of ["$referrer", "$initial_referrer", "$session_entry_referrer"]) {
    if (typeof event.properties[name] === "string") {
      try { event.properties[name] = new URL(event.properties[name]).origin; } catch { event.properties[name] = "direct"; }
    }
  }
  for (const key of Object.keys(event.properties)) {
    if (/^(\$initial_|\$session_entry_)?(utm_|gclid|fbclid|msclkid)/.test(key) || /\$(initial_)?(pathname|host|raw_user_agent|ip)$/.test(key)) delete event.properties[key];
  }
  return event;
}

export async function setAnalytics(config: AnalyticsConfig, choice: Consent) {
  const current = ++generation;
  consent = config.enabled && !privacySignal() ? choice : "essential";
  if (consent === "essential") {
    pageviewSent = false;
    pending = [];
    client?.stopSessionRecording();
    client?.opt_out_capturing();
    return;
  }
  if (!config.enabled) return;
  try {
    const posthog = (await import("posthog-js")).default;
    if (current !== generation || (consent as Consent) === "essential") return;
    if (!client) {
      client = posthog.init(config.key, {
        api_host: config.host,
        defaults: "2026-01-30",
        persistence: "localStorage",
        cross_subdomain_cookie: false,
        person_profiles: "never",
        opt_out_capturing_by_default: true,
        opt_out_persistence_by_default: true,
        respect_dnt: true,
        ip: false,
        autocapture: false,
        capture_pageview: false,
        capture_pageleave: true,
        capture_dead_clicks: false,
        capture_heatmaps: false,
        capture_performance: false,
        capture_exceptions: false,
        disable_surveys: true,
        enable_recording_console_log: false,
        disable_session_recording: true,
        session_recording: {
          maskAllInputs: true,
          maskTextSelector: "[data-private]",
          maskAttributeFn: (name, value) => ["href", "src", "action"].includes(name)
            ? cleanUrl(value) : value,
          blockSelector: ".ph-no-capture",
          recordCrossOriginIframes: false,
          recordHeaders: false,
          recordBody: false,
          captureCanvas: { recordCanvas: false },
          captureJsonLd: false,
          maskCapturedNetworkRequestFn: request => request.isInitial || !request.method
            ? { ...request, name: cleanUrl(request.name) } : null,
        },
        before_send: sanitize,
      });
    }
    if (!client) return;
    client.opt_in_capturing({ captureEventName: false });
    if (consent === "replay") client.startSessionRecording();
    else client.stopSessionRecording();
    if (!pageviewSent) {
      client.capture("$pageview", { site_version: "interactive-v1" });
      pageviewSent = true;
    }
    for (const item of pending.splice(0)) client.capture(item.name, item.properties);
  } catch {
    // Analytics failure must never interrupt a download or the demo.
    pending = [];
  }
}

export function track(name: string, properties: Properties = {}) {
  if (consent === "essential" || privacySignal()) return;
  const payload = { site_version: "interactive-v1", ...properties };
  if (client && !client.has_opted_out_capturing()) client.capture(name, payload);
  else if (pending.length < 20) pending.push({ name, properties: payload });
}

/** Only fixed actions are collected: no search queries, field values or node text. */
export function observeDemo(iframe: HTMLIFrameElement) {
  const doc = iframe.contentDocument;
  if (!doc) return () => {};
  let drag: { x: number; y: number; kind: string } | null = null;
  let lastZoom = 0;
  const source = () => doc.querySelector('.demo-tabs button[aria-pressed="true"]')?.textContent === "OpenAPI" ? "openapi" : "database";
  const click = (event: Event) => {
    const target = event.target as Element | null;
    const button = target?.closest("button");
    if (!button) return;
    const label = button.getAttribute("aria-label") ?? button.textContent?.trim() ?? "";
    const action = button.closest(".demo-tabs") ? "source_changed"
      : label.startsWith("Inspect ") ? "node_inspected"
      : ({ "Reset demo": "reset", "Auto layout": "auto_layout", "Fit graph to screen": "fit", "Zoom in": "zoom", "Zoom out": "zoom", "Toggle minimap": "minimap" } as Record<string, string>)[label];
    if (action) track("demo_interaction", { action, source: source() });
  };
  const change = (event: Event) => {
    if ((event.target as Element)?.getAttribute("aria-label") === "Inspect a node") track("demo_interaction", { action: "node_inspected", source: source() });
  };
  const down = (event: PointerEvent) => {
    const target = event.target as Element;
    if (!target.closest('[aria-label="Interactive schema graph"]')) return;
    drag = { x: event.clientX, y: event.clientY, kind: target.closest(".entity-node") ? "node_moved" : target.closest('[aria-label^="Move group "]') ? "group_moved" : "pan" };
  };
  const up = (event: PointerEvent) => {
    if (drag && Math.hypot(event.clientX - drag.x, event.clientY - drag.y) > 8) track("demo_interaction", { action: drag.kind, source: source() });
    drag = null;
  };
  const wheel = () => {
    if (Date.now() - lastZoom < 3000) return;
    lastZoom = Date.now();
    track("demo_interaction", { action: "zoom", source: source() });
  };
  doc.addEventListener("click", click);
  doc.addEventListener("change", change);
  doc.addEventListener("pointerdown", down, true);
  doc.addEventListener("pointerup", up, true);
  const graphWheel = (event: WheelEvent) => {
    if ((event.target as Element)?.closest('[aria-label="Interactive schema graph"]')) wheel();
  };
  doc.addEventListener("wheel", graphWheel, { passive: true });
  return () => {
    doc.removeEventListener("click", click);
    doc.removeEventListener("change", change);
    doc.removeEventListener("pointerdown", down, true);
    doc.removeEventListener("pointerup", up, true);
    doc.removeEventListener("wheel", graphWheel);
  };
}
