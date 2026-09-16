import { createReleaseLoader, type ReleaseStorage } from "@/lib/releases";

export const dynamic = "force-dynamic";
const loadRelease = createReleaseLoader();

export async function GET(request: Request) {
  // The edge cache survives isolate recycling. Local runtimes without it use memory.
  const cache = typeof caches !== "undefined" ? (caches as CacheStorage & { default?: Cache }).default : undefined;
  const key = new Request(new URL("/__schematlas-cache/latest-release-v1", request.url));
  const storage: ReleaseStorage | undefined = cache ? {
    read: async () => (await cache.match(key))?.json(),
    write: async (entry, ttl) => {
      await cache.put(key, Response.json(entry, { headers: { "Cache-Control": `public, max-age=${ttl}` } }));
    },
  } : undefined;
  const release = await loadRelease(storage);
  return Response.json(release, { headers: { "Cache-Control": "public, max-age=60" } });
}
