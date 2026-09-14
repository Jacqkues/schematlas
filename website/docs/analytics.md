# Landing page analytics

PostHog is loaded only after consent. The config route returns `enabled: false`
unless a valid public project key and EU/US region are configured. Missing config,
blocked analytics, or SDK failures never prevent downloads or graph interactions.

## Activate

Create an EU PostHog project, enable Session Replay in that project's settings, and
set these runtime variables in Sites, then deploy the saved site version:

- `POSTHOG_PUBLIC_KEY`: browser ingestion key starting with `phc_`.
- `POSTHOG_REGION`: `eu` (uses `https://eu.i.posthog.com`).

Do not use a personal/admin API key. Local development reads the same variables
from `.env.local` or Cloudflare bindings in `.dev.vars`; both files are ignored.
Only the public ingestion key and selected region are returned to the browser.

## Consent and recording

The compact banner offers Accept (analytics and recordings), Reject, and Settings.
Settings retains Reject optional, Analytics only, and Analytics + recordings.
No PostHog SDK, identifiers, events or recordings are loaded before a choice.
The choice is remembered for 180 days, can be changed using the footer's Privacy
settings, and synchronizes across tabs. Do Not Track and Global Privacy Control
keep tracking disabled. Withdrawing consent stops future collection and clears
PostHog's stored identifiers; it cannot retract data already transmitted.

Recording uses PostHog's project sampling/retention settings. Start with the free
plan and monitor Replay usage (5,000 recordings/month at implementation time).
The UI does not override project recording limits or sampling decisions.

Inputs are masked, query strings removed, personal profiles disabled, and console,
network payloads and shader WebGL canvas recording are disabled. The same-origin
demo is ordinary DOM/SVG and can be replayed along with the landing page. Demo
events contain action names and source type, never search text or table fields.
No SDK is installed in the downloadable desktop app or standalone demo tab.

## Events and useful dashboards

| Event | Purpose / properties |
| --- | --- |
| `$pageview`, `$pageleave` | Visitors, sessions, device/browser and referring origin |
| `section_viewed` | Sections reached: workspace, explore, download |
| `download_cta_clicked` | Hero-to-download-section intent; location |
| `download_clicked` | Installer link click; platform, architecture, format, release, location |
| `github_clicked` | Repository/documentation/releases/issues clicks and location |
| `demo_loaded`, `demo_load_failed`, `demo_retry` | Demo loading experience |
| `demo_opened_fullscreen` | Opening the standalone workspace |
| `demo_interaction` | Inspection, source switching, dragging, pan/zoom, reset, layout and minimap |

Every custom event includes `site_version: interactive-v1` for future landing-page
comparisons. Section views occur once per consent lifecycle; wheel events are
limited to one every three seconds. PostHog batches transport requests.

Suggested dashboard: unique visitors by day and referring domain; a same-session
funnel from pageview → workspace section → demo interaction → download click;
installer clicks by platform; replay sessions filtered by download clicks or
demo load failures. Counts reflect consenting visitors and may miss blockers.
Download clicks measure intent, not completed downloads or unique installers.
GitHub release asset `download_count` is a separate aggregate if needed later.

## Verify

Run `npm run test:analytics` with the site running locally. Tests intercept all
PostHog traffic and use a fake ingestion key, so they send nothing to a real
project. They check consent, recordings, masked input, URL redaction, conversions,
withdrawal, cross-tab choices, and browser privacy signals.

With a real configured project, accept the desired tracking mode and verify the
pageview and download events in PostHog Activity, then a recording in Replay.
The dashboard and account-side settings require access to your PostHog project.
