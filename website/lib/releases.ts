export const REPOSITORY = "https://github.com/Jacqkues/schematlas";
export const LATEST_RELEASE = `${REPOSITORY}/releases/latest`;
export const GITHUB_RELEASE_API = "https://api.github.com/repos/Jacqkues/schematlas/releases/latest";
export const DOWNLOAD_KINDS = [
  "macos-arm64.dmg", "macos-x64.dmg", "windows-x64.exe",
  "windows-x64.msi", "linux-x64.AppImage", "linux-x64.deb",
] as const;
export type DownloadKind = typeof DOWNLOAD_KINDS[number];
export type ReleaseInfo = { tag: string | null; url: string; assets: Partial<Record<DownloadKind, string>> };
export const FALLBACK_RELEASE: ReleaseInfo = { tag: null, url: LATEST_RELEASE, assets: {} };
const FRESH_MS = 10 * 60 * 1000;
const RETRY_MS = 60 * 1000;
const record = (value: unknown): Record<string, unknown> | null =>
  value !== null && typeof value === "object" && !Array.isArray(value) ? value as Record<string, unknown> : null;

/** Only stable tags and real uploaded assets from this repository become links. */
export function parseReleaseInfo(value: unknown): ReleaseInfo | null {
  const release = record(value);
  if (!release) return null;
  if (release.tag === null && release.url === LATEST_RELEASE) return FALLBACK_RELEASE;
  if (typeof release.tag !== "string" || !/^v?\d+\.\d+\.\d+$/.test(release.tag)) return null;
  if (release.url !== `${REPOSITORY}/releases/tag/${release.tag}`) return null;
  const input = record(release.assets);
  if (!input) return null;
  const assets: ReleaseInfo["assets"] = {};
  const prefix = `${REPOSITORY}/releases/download/${release.tag}/`;
  for (const kind of DOWNLOAD_KINDS) {
    const url = input[kind];
    if (typeof url !== "string" || !url.startsWith(prefix)) continue;
    const filename = url.slice(prefix.length);
    if (/^[A-Za-z0-9._-]+$/.test(filename) && filename.endsWith(`-${kind}`)) assets[kind] = url;
  }
  return { tag: release.tag, url: release.url, assets };
}

export function parseGitHubRelease(value: unknown): ReleaseInfo | null {
  const release = record(value);
  if (!release || release.draft !== false || release.prerelease !== false || !Array.isArray(release.assets)) return null;
  const assets: ReleaseInfo["assets"] = {};
  for (const kind of DOWNLOAD_KINDS) {
    const candidates = release.assets.map(record).filter(asset => asset
      && typeof asset.name === "string" && asset.name.endsWith(`-${kind}`)
      && asset.state === "uploaded" && typeof asset.size === "number" && asset.size > 0);
    if (candidates.length === 1 && typeof candidates[0]?.browser_download_url === "string") {
      assets[kind] = candidates[0].browser_download_url;
    }
  }
  return parseReleaseInfo({ tag: release.tag_name, url: release.html_url, assets });
}

type CacheEntry = { release: ReleaseInfo; expiresAt: number };
export type ReleaseStorage = {
  read(): Promise<unknown>;
  write(entry: CacheEntry, ttlSeconds: number): Promise<void>;
};

/** One bounded cache entry and one in-flight request per Worker isolate. */
export function createReleaseLoader(fetcher: typeof fetch = fetch, now: () => number = Date.now) {
  let cached: CacheEntry | undefined;
  let pending: Promise<ReleaseInfo> | undefined;
  return async function load(storage?: ReleaseStorage): Promise<ReleaseInfo> {
    if (cached && cached.expiresAt > now()) return cached.release;
    if (pending) return pending;
    pending = (async () => {
      try {
        const entry = record(await storage?.read());
        const release = parseReleaseInfo(entry?.release);
        if (release && typeof entry?.expiresAt === "number" && entry.expiresAt > now() && entry.expiresAt <= now() + FRESH_MS) {
          cached = { release, expiresAt: entry.expiresAt };
          return release;
        }
      } catch { /* Cache availability must not block downloads. */ }
      let release = FALLBACK_RELEASE;
      try {
        const response = await fetcher(GITHUB_RELEASE_API, {
          headers: { Accept: "application/vnd.github+json", "User-Agent": "Schematlas-Website", "X-GitHub-Api-Version": "2022-11-28" },
          signal: AbortSignal.timeout(4000),
          redirect: "manual",
        });
        if (response.ok) release = parseGitHubRelease(await response.json()) ?? FALLBACK_RELEASE;
      } catch { /* Timeouts, rate limits and outages keep the GitHub fallback usable. */ }
      const ttl = release.tag ? FRESH_MS : RETRY_MS;
      cached = { release, expiresAt: now() + ttl };
      try { await storage?.write(cached, ttl / 1000); } catch { /* Memory caching still works. */ }
      return release;
    })();
    try { return await pending; } finally { pending = undefined; }
  };
}
