import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { gunzipSync } from "node:zlib";
import { chromium } from "playwright-core";

const base = process.env.ANALYTICS_TEST_URL ?? "http://localhost:3000";
const key = "phc_schematlas_test_only";
const config = { enabled: true, key, host: "https://eu.i.posthog.com", region: "eu" };
const consentKey = "schematlas.analytics-consent.v1";
const remoteConfig = { hasFeatureFlags: false, autocapture_opt_out: true, sessionRecording: { endpoint: "/s/", sampleRate: 1, minimumDurationMilliseconds: 0 }, capturePerformance: false, supportedCompression: ["gzip-js"] };
const browser = await chromium.launch({ channel: "chrome", headless: true });
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
async function until(test, message, timeout = 12000) {
  const end = Date.now() + timeout;
  while (Date.now() < end) { if (await test()) return; await sleep(150); }
  assert.fail(message);
}
function decode(request) {
  const buffer = request.postDataBuffer();
  if (!buffer) return [];
  let value;
  try { value = JSON.parse(buffer.toString()); }
  catch {
    try { value = JSON.parse(gunzipSync(buffer).toString()); }
    catch {
      const data = new URLSearchParams(buffer.toString()).get("data");
      assert.ok(data, `Unexpected analytics encoding: ${request.url()}`);
      const decoded = Buffer.from(data, "base64");
      try { value = JSON.parse(decoded.toString()); }
      catch { value = JSON.parse(gunzipSync(decoded).toString()); }
    }
  }
  return Array.isArray(value) ? value : value.batch ?? [value];
}
function expandReplay(value) {
  if (typeof value === "string" && value.charCodeAt(0) === 31 && value.charCodeAt(1) === 139) {
    return expandReplay(JSON.parse(gunzipSync(Buffer.from(value, "latin1")).toString()));
  }
  if (Array.isArray(value)) return value.map(expandReplay);
  if (value && typeof value === "object") return Object.fromEntries(Object.entries(value).map(([key, item]) => [key, expandReplay(item)]));
  return value;
}
async function setup({ configured = true, dnt = false, gpc = false } = {}) {
  const context = await browser.newContext({ viewport: { width: 1440, height: 1000 }, reducedMotion: "reduce",
    userAgent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36" });
  const events = [], requests = [], errors = [], logs = [];
  await context.addInitScript(({ dnt, gpc }) => {
    // Exercise visitor behavior; the SDK deliberately excludes automated browsers.
    Object.defineProperty(navigator, "webdriver", { get: () => false });
    Object.defineProperty(navigator, "userAgentData", { get: () => undefined });
    if (dnt) Object.defineProperty(navigator, "doNotTrack", { value: "1" });
    if (gpc) Object.defineProperty(navigator, "globalPrivacyControl", { value: true });
  }, { dnt, gpc });
  await context.route("**/api/releases/latest", route => route.fulfill({ json: {
    tag: "v9.8.7", url: "https://github.com/Jacqkues/schematlas/releases/tag/v9.8.7",
    assets: { "macos-arm64.dmg": "https://github.com/Jacqkues/schematlas/releases/download/v9.8.7/Schematlas-v9.8.7-macos-arm64.dmg" },
  } }));
  await context.route("**/api/analytics-config", route => route.fulfill({ json: configured ? config : { enabled: false } }));
  await context.route(/^https:\/\/[^/]*posthog\.com\//, async route => {
    const request = route.request();
    requests.push(request.url());
    const url = new URL(request.url());
    if (request.method() === "OPTIONS") return route.fulfill({ status: 200, headers: { "Access-Control-Allow-Origin": "*", "Access-Control-Allow-Headers": "*" } });
    if (url.pathname.endsWith("config.js")) return route.fulfill({ contentType: "application/javascript", body: `window._POSTHOG_REMOTE_CONFIG = { ${JSON.stringify(key)}: {config: ${JSON.stringify(remoteConfig)}} };` });
    if (url.pathname.endsWith("/config")) return route.fulfill({ json: remoteConfig });
    if (url.pathname.includes("/static/") && url.pathname.endsWith(".js")) {
      const name = url.pathname.split("/").at(-1);
      assert.ok(["recorder.js", "recorder-v2.js", "posthog-recorder.js", "lazy-recorder.js"].includes(name), `Unexpected SDK extension: ${name}`);
      return route.fulfill({ contentType: "application/javascript", body: await readFile(new URL(`../node_modules/posthog-js/dist/${name}`, import.meta.url), "utf8") });
    }
    if (url.pathname.includes("/flags")) return route.fulfill({ json: { featureFlags: {}, featureFlagPayloads: {} } });
    if (request.method() === "POST") events.push(...decode(request));
    return route.fulfill({ json: { status: 1 } });
  });
  const page = await context.newPage();
  page.on("pageerror", error => errors.push(error.message));
  page.on("console", message => logs.push(message.text()));
  const configuredResponse = page.waitForResponse(response => response.url().endsWith("/api/analytics-config"));
  await page.goto(`${base}/?token=do-not-transmit&utm_source=do-not-transmit`);
  await configuredResponse;
  return { context, page, events, requests, errors, logs };
}

try {
  const rejected = await setup();
  await rejected.page.getByRole("button", { name: "Reject", exact: true }).waitFor();
  assert.equal(rejected.requests.length, 0, "No PostHog request before consent");
  await rejected.page.getByRole("button", { name: "Reject", exact: true }).click();
  await rejected.page.reload();
  await sleep(800);
  assert.equal(rejected.requests.length, 0, "Reject persists without loading PostHog");
  await rejected.context.close();

  const analytics = await setup();
  await analytics.page.getByRole("button", { name: "Settings", exact: true }).click();
  await analytics.page.getByRole("button", { name: "Analytics only", exact: true }).click();
  await until(() => analytics.events.some(e => e.event === "$pageview"), "Pageview was not captured").catch(error => { console.log({requests: analytics.requests, errors: analytics.errors, logs: analytics.logs}); throw error; });
  assert.equal(analytics.events.filter(e => e.event === "$pageview").length, 1, "Initial pageview is not duplicated");
  await analytics.page.locator('a[href$="macos-arm64.dmg"]').evaluate(anchor => anchor.addEventListener("click", e => e.preventDefault()));
  await analytics.page.locator('a[href$="macos-arm64.dmg"]').click();
  await until(() => analytics.events.some(e => e.event === "download_clicked"), "Download event missing");
  const download = analytics.events.find(e => e.event === "download_clicked");
  assert.equal(download.properties.platform, "macos");
  assert.equal(download.properties.architecture, "arm64");
  assert.equal(download.properties.release, "v9.8.7");
  assert.ok(download.properties.$session_id, "Events share a session identifier");
  assert.equal(analytics.events.some(e => e.event === "$snapshot"), false, "Analytics-only never sends recordings");
  assert.equal(analytics.requests.some(url => url.includes("recorder")), false, "Recorder is not downloaded for analytics-only");
  assert.equal(JSON.stringify(analytics.events).includes("do-not-transmit"), false,
    "URL parameters are stripped: " + JSON.stringify(analytics.events.map(e => Object.entries(e.properties).filter(([, value]) => JSON.stringify(value)?.includes("do-not-transmit")).map(([key]) => key))));
  await analytics.page.getByRole("button", { name: "Privacy settings", exact: true }).click();
  await analytics.page.getByRole("button", { name: "Reject optional", exact: true }).click();
  await sleep(1000);
  const afterWithdrawal = analytics.events.length;
  await analytics.page.locator('a[href$="macos-arm64.dmg"]').click();
  await sleep(3500);
  assert.equal(analytics.events.length, afterWithdrawal, "Withdrawal stops new events");
  await analytics.page.getByRole("button", { name: "Privacy settings", exact: true }).click();
  await analytics.page.getByText("What is collected?", { exact: true }).click();
  await analytics.page.getByRole("button", { name: "Reset saved choice", exact: true }).click();
  await analytics.page.getByRole("button", { name: "Accept", exact: true }).waitFor();
  assert.equal(await analytics.page.evaluate(key => localStorage.getItem(key), consentKey), null, "Reset removes the saved choice");
  await analytics.page.reload();
  await analytics.page.getByRole("button", { name: "Reject", exact: true }).waitFor();
  await sleep(800);
  assert.equal(analytics.events.length, afterWithdrawal, "Reset keeps tracking off and restores the compact banner after reload");
  assert.deepEqual(analytics.errors, []);
  await analytics.context.close();
  console.log("PASS: no pre-consent tracking, persistent rejection, analytics-only, download/session metadata, URL sanitization and withdrawal.");

  const replay = await setup();
  await replay.page.getByRole("button", { name: "Accept", exact: true }).click();
  await replay.page.locator(".workspace-demo").scrollIntoViewIfNeeded();
  await replay.page.locator(".demo-iframe.is-ready").waitFor();
  const frame = replay.page.frameLocator(".demo-iframe");
  await until(() => replay.events.some(e => e.event === "$snapshot"), "Session replay was not recorded", 20000);
  await frame.getByLabel("Search sample schema").fill("private-input-must-be-masked");
  await sleep(500);
  await frame.getByLabel("Search sample schema").fill("");
  await frame.getByRole("button", { name: "OpenAPI", exact: true }).click();
  await until(() => replay.events.some(e => e.event === "$snapshot"), "Session replay was not recorded", 20000);
  await until(() => replay.events.some(e => e.event === "demo_interaction" && e.properties.action === "source_changed"), "Demo source event missing");
  const replayText = () => JSON.stringify(expandReplay(replay.events.filter(e => e.event === "$snapshot")));
  await until(() => replayText().includes("Search sample schema"), "The same-origin demo is present in the recording");
  assert.equal(replayText().includes("private-input-must-be-masked"), false, "Inputs are masked in decoded recordings");
  assert.equal(replayText().includes("do-not-transmit"), false, "Recording URLs are redacted: " + replayText().match(/.{0,100}do-not-transmit.{0,80}/g)?.slice(0,6).join(" / "));
  assert.equal(JSON.stringify(replay.events.filter(e => e.event !== "$snapshot")).includes("private-input"), false);
  await replay.page.getByRole("button", { name: "Privacy settings", exact: true }).click();
  await replay.page.getByRole("button", { name: "Analytics only", exact: true }).click();
  await sleep(1000);
  const snapshots = replay.events.filter(e => e.event === "$snapshot").length;
  await replay.page.mouse.move(100,100);
  await replay.page.mouse.wheel(0,100);
  await sleep(4000);
  assert.equal(replay.events.filter(e => e.event === "$snapshot").length, snapshots, "Recording stops when only analytics is selected");
  await replay.page.evaluate(key => localStorage.setItem(key, JSON.stringify({ choice: "essential", at: Date.now() })), consentKey);
  const second = await replay.context.newPage();
  await second.goto(base);
  await sleep(500);
  await second.evaluate(key => localStorage.setItem(key, JSON.stringify({ choice: "essential", at: Date.now() + 1 })), consentKey);
  await sleep(500);
  const before = replay.events.length;
  await frame.getByRole("button", { name: "Database", exact: true }).click();
  await sleep(3500);
  assert.equal(replay.events.length, before, "Another tab can revoke consent");
  assert.deepEqual(replay.errors, []);
  await replay.context.close();
  console.log("PASS: opt-in recording, demo interaction, replay withdrawal and cross-tab revocation.");

  for (const settings of [{ dnt: true }, { gpc: true }, { configured: false }]) {
    const test = await setup(settings);
    await test.page.getByRole("button", { name: "Privacy settings", exact: true }).click();
    await test.page.locator(".analytics-preferences").waitFor();
    await sleep(500);
    assert.equal(await test.page.getByRole("button", { name: "Analytics + recordings", exact: true }).count(), 0);
    assert.equal(test.requests.length, 0);
    await test.context.close();
  }
  console.log("PASS: DNT, GPC and missing configuration keep collection disabled.");
} finally { await browser.close(); }
