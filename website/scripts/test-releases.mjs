import assert from "node:assert/strict";
import { chromium } from "playwright-core";
import { createReleaseLoader, parseGitHubRelease, parseReleaseInfo, DOWNLOAD_KINDS, FALLBACK_RELEASE, REPOSITORY } from "../lib/releases.ts";

function fixture(tag = "v9.8.7") {
  return {
    tag_name: tag, html_url: `${REPOSITORY}/releases/tag/${tag}`, draft: false, prerelease: false,
    assets: DOWNLOAD_KINDS.map(kind => ({
      name: `Schematlas-${tag}-${kind}`, state: "uploaded", size: 100,
      browser_download_url: `${REPOSITORY}/releases/download/${tag}/Schematlas-${tag}-${kind}`,
    })),
  };
}
const parsed = parseGitHubRelease(fixture());
assert.equal(parsed.tag, "v9.8.7");
assert.equal(Object.keys(parsed.assets).length, 6);
assert.equal(parseGitHubRelease({ ...fixture(), draft: true }), null);
assert.equal(parseGitHubRelease({ ...fixture(), prerelease: true }), null);
assert.equal(parseGitHubRelease(fixture("v9.9.0-rc1")), null);
assert.equal(parseGitHubRelease({ ...fixture(), html_url: "https://example.com" }), null);
assert.equal(parseGitHubRelease(null), null);
const partial = fixture();
partial.assets[0].browser_download_url = "https://github.com.evil.example/installer.dmg";
partial.assets[1].browser_download_url += "?redirect=evil";
partial.assets[2].size = 0;
partial.assets.push({ ...partial.assets[3] });
assert.deepEqual(Object.keys(parseGitHubRelease(partial).assets), ["linux-x64.AppImage", "linux-x64.deb"]);
assert.deepEqual(parseReleaseInfo({ ...parsed, assets: { "macos-arm64.dmg": "javascript:alert(1)" } }).assets, {});

let time = 0, calls = 0, version = "v9.8.7", stored;
const storage = { read: async () => stored, write: async entry => { stored = entry; } };
const fetcher = async (_url, options) => {
  assert.equal(options.redirect, "manual", "Use an edge-compatible redirect policy");
  calls++; return Response.json(fixture(version));
};
const load = createReleaseLoader(fetcher, () => time);
const results = await Promise.all(Array.from({ length: 8 }, () => load(storage)));
assert.equal(calls, 1, "Concurrent visitors share one upstream request");
assert.ok(results.every(result => result.tag === version));
await load(storage);
assert.equal(calls, 1, "Memory cache avoids another request");
const recycled = createReleaseLoader(fetcher, () => time);
await recycled(storage);
assert.equal(calls, 1, "Edge cache survives a new isolate");
time = 600001; version = "v9.8.8";
assert.equal((await recycled(storage)).tag, version);
assert.equal(calls, 2, "Cache expiry discovers a new release without changing code");
let failedCalls = 0;
const recovering = createReleaseLoader(async () => {
  failedCalls++;
  return failedCalls === 1 ? new Response("Rate limited", { status: 429 }) : Response.json(fixture());
}, () => time);
assert.deepEqual(await recovering(), FALLBACK_RELEASE);
assert.deepEqual(await recovering(), FALLBACK_RELEASE);
assert.equal(failedCalls, 1, "Failures are cached briefly");
time += 60001;
assert.equal((await recovering()).tag, "v9.8.7");
const offline = createReleaseLoader(async () => { throw new Error("Offline"); });
assert.deepEqual(await offline(), FALLBACK_RELEASE);
const invalid = createReleaseLoader(async () => new Response("not json"));
assert.deepEqual(await invalid(), FALLBACK_RELEASE);
const brokenCache = { read: async () => { throw new Error("cache unavailable"); }, write: async () => { throw new Error("cache unavailable"); } };
assert.equal((await createReleaseLoader(fetcher)(brokenCache)).tag, version);
console.log("PASS: stable release parsing, safe/missing/ambiguous assets, deduplication, memory/edge caching, expiry, rate limits and recovery.");

const base = process.env.DEMO_TEST_URL ?? "http://localhost:3000";
const browser = await chromium.launch({ channel: "chrome", headless: true });
try {
  const page = await browser.newPage({ reducedMotion: "reduce" });
  const errors = [];
  page.on("pageerror", error => errors.push(error.message));
  let requests = 0;
  await page.route("**/api/releases/latest", async route => {
    requests++;
    await route.fulfill({ json: parsed });
  });
  await page.goto(base);
  await page.getByRole("link", { name: "VERSION 9.8.7" }).waitFor();
  assert.equal(requests, 1, "All version labels and links share one browser request");
  assert.equal(await page.getByRole("link", { name: "Release notes · v9.8.7" }).getAttribute("href"), parsed.url);
  assert.deepEqual(await page.locator(".download-link").evaluateAll(links => links.map(link => link.href)), DOWNLOAD_KINDS.map(kind => parsed.assets[kind]));
  await page.unroute("**/api/releases/latest");
  await page.route("**/api/releases/latest", route => route.fulfill({ json: { ...parsed, assets: {} } }));
  await page.reload();
  await page.getByRole("link", { name: "VERSION 9.8.7" }).waitFor();
  assert.ok((await page.locator(".download-link").evaluateAll(links => links.map(link => link.href))).every(url => url === parsed.url));
  await page.unroute("**/api/releases/latest");
  await page.route("**/api/releases/latest", route => route.abort());
  await page.reload();
  await page.getByRole("link", { name: "LATEST RELEASE", exact: true }).waitFor();
  assert.ok((await page.locator(".download-link").evaluateAll(links => links.map(link => link.href))).every(url => url === FALLBACK_RELEASE.url));
  const context = await browser.newContext({ javaScriptEnabled: false });
  const noJs = await context.newPage();
  await noJs.goto(base);
  assert.equal(await noJs.locator(".download-link").count(), 6);
  assert.ok((await noJs.locator(".download-link").evaluateAll(links => links.map(link => link.href))).every(url => url === FALLBACK_RELEASE.url));
  await context.close();
  assert.deepEqual(errors, []);
  console.log("PASS: dynamic version and six download links, one shared request, missing-asset and network fallback, and usable links without JavaScript.");
} finally { await browser.close(); }
