import { settings } from "@/lib/settings";
import type { Point } from "./layout";

const layoutCache = new Map<string, Record<string, Point>>();
const inflightLoads = new Map<string, Promise<Record<string, Point>>>();
const persistTimers = new Map<string, ReturnType<typeof setTimeout>>();

function layoutKeyOf(path: string): string {
  return `layout:${path}`;
}

export function getCachedEntityLayout(
  path: string,
): Record<string, Point> | undefined {
  return layoutCache.get(layoutKeyOf(path));
}

export function preloadEntityLayout(
  path: string,
): Promise<Record<string, Point>> {
  const key = layoutKeyOf(path);
  const cached = layoutCache.get(key);
  if (cached) return Promise.resolve(cached);

  const pending = inflightLoads.get(key);
  if (pending) return pending;

  const promise = settings
    .get<Record<string, Point>>(key)
    .then((value) => {
      // 若加载期间已有本地拖拽写入，保留最新缓存而不被旧盘面覆盖
      const latest = layoutCache.get(key);
      if (latest) return latest;
      const resolved = value ?? {};
      layoutCache.set(key, resolved);
      return resolved;
    })
    .catch((error) => {
      console.error("load entity layout failed:", error);
      const fallback = layoutCache.get(key) ?? {};
      layoutCache.set(key, fallback);
      return fallback;
    })
    .finally(() => {
      inflightLoads.delete(key);
    });

  inflightLoads.set(key, promise);
  return promise;
}

export function persistEntityPosition(
  path: string,
  id: string,
  pos: Point,
): void {
  const key = layoutKeyOf(path);
  const next = { ...(layoutCache.get(key) ?? {}), [id]: pos };
  layoutCache.set(key, next);

  const existingTimer = persistTimers.get(key);
  if (existingTimer !== undefined) clearTimeout(existingTimer);
  const timer = setTimeout(() => {
    persistTimers.delete(key);
    const latest = layoutCache.get(key);
    if (latest) {
      void settings.set(key, latest).catch(console.error);
    }
  }, 250);
  persistTimers.set(key, timer);
}
