import { useEffect, useSyncExternalStore } from "react";

import type { Release } from "./model";

const subscribe = (listener: () => void) => {
  window.addEventListener("popstate", listener);
  return () => window.removeEventListener("popstate", listener);
};
export const useRoute = () => {
  const search = useSyncExternalStore(subscribe, () => location.search);
  const params = new URLSearchParams(search);
  const requested = params.get("view") ?? "browse";
  const view = ["browse", "saved", "publishers", "guide"].includes(requested)
    ? requested
    : "browse";
  const rawOffset = Number(params.get("offset"));
  const offset =
    Number.isSafeInteger(rawOffset) && rawOffset >= 0
      ? Math.floor(rawOffset / 30) * 30
      : 0;
  return {
    id: params.get("plugin"),
    license: params.get("license") ?? "",
    offset,
    publisher: params.get("publisher") ?? "",
    q: params.get("q") ?? "",
    sample: params.get("catalog") === "sample",
    search,
    version: params.get("version"),
    view,
  };
};
export type Route = ReturnType<typeof useRoute>;
export const routeUrl = (
  changes: Record<string, string | null>,
  base = location.search
) => {
  const params = new URLSearchParams(base);
  for (const [key, value] of Object.entries(changes)) {
    if (value) {
      params.set(key, value);
    } else {
      params.delete(key);
    }
  }
  return params.size ? `/?${params}` : "/";
};
export const browseUrl = (changes: Record<string, string | null> = {}) =>
  routeUrl({ ...changes, plugin: null, version: null });
export const releaseUrl = (release: Release) =>
  routeUrl({ plugin: release.plugin_id, version: release.version });
export const rootUrl = (changes: Record<string, string | null> = {}) =>
  routeUrl(
    changes,
    new URLSearchParams(location.search).get("catalog") === "sample"
      ? "?catalog=sample"
      : ""
  );
export const publisherUrl = (publisher: string) => rootUrl({ publisher });
export const navigate = (url: string) => {
  if (`${location.pathname}${location.search}` === url) {
    return;
  }
  history.pushState(null, "", url);
  window.dispatchEvent(new PopStateEvent("popstate"));
};
export const followLink = (event: MouseEvent) => {
  if (
    event.defaultPrevented ||
    event.button !== 0 ||
    event.metaKey ||
    event.ctrlKey ||
    event.shiftKey ||
    event.altKey
  ) {
    return;
  }
  const { target } = event;
  if (!(target instanceof Element) || !target.closest(".marketplace")) {
    return;
  }
  const link = target.closest("a");
  if (!link || link.target || link.hasAttribute("download")) {
    return;
  }
  const url = new URL(link.href);
  if (url.origin !== location.origin || url.pathname !== "/") {
    return;
  }
  event.preventDefault();
  navigate(`${url.pathname}${url.search}`);
};

export const useClientNavigation = () => {
  useEffect(() => {
    document.addEventListener("click", followLink);
    return () => document.removeEventListener("click", followLink);
  }, []);
};
