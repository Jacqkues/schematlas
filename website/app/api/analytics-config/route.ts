import { env } from "cloudflare:workers";

export const dynamic = "force-dynamic";

export function GET() {
  const bindings = env as { POSTHOG_PUBLIC_KEY?: string; POSTHOG_REGION?: string };
  const key = (bindings.POSTHOG_PUBLIC_KEY ?? process.env.POSTHOG_PUBLIC_KEY)?.trim();
  const region = (bindings.POSTHOG_REGION ?? process.env.POSTHOG_REGION)?.toLowerCase();
  // This is the browser ingestion key only, never a personal/admin API key.
  const configured = Boolean(key && /^phc_[a-zA-Z0-9_-]+$/.test(key) && (region === "eu" || region === "us"));
  return Response.json(configured ? {
    enabled: true,
    key,
    host: region === "eu" ? "https://eu.i.posthog.com" : "https://us.i.posthog.com",
    region,
  } : { enabled: false }, { headers: { "Cache-Control": "no-store" } });
}
