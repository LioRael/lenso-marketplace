import { useEffect, useState } from "react";

import { loadKeylessCatalog, selectCatalog } from "./keyless";
import type { Catalog } from "./model";
import { sampleResponse } from "./sample";

interface Result {
  data?: Catalog;
  error?: string;
  status?: number;
}
export const useKeylessDirectory = (enabled: boolean) => {
  const [result, setResult] = useState<Result>();
  const [pending, setPending] = useState(false);
  const [attempt, setAttempt] = useState(0);
  useEffect(() => {
    if (!enabled) {
      return;
    }
    let controller: AbortController | undefined;
    const load = async () => {
      controller?.abort();
      controller = new AbortController();
      const { signal } = controller;
      setResult(undefined);
      setPending(true);
      try {
        const data = await loadKeylessCatalog(signal);
        if (!signal.aborted) {
          setResult({ data });
        }
      } catch {
        if (!signal.aborted) {
          setResult({
            error:
              "The current catalog could not be confirmed. Check your connection and try again.",
          });
        }
      } finally {
        if (!signal.aborted) {
          setPending(false);
        }
      }
    };
    const refreshVisible = () => {
      if (document.visibilityState === "visible") {
        void load();
      }
    };
    void load();
    document.addEventListener("visibilitychange", refreshVisible);
    return () => {
      controller?.abort();
      document.removeEventListener("visibilitychange", refreshVisible);
    };
  }, [enabled, attempt]);
  return {
    data: enabled ? result?.data : undefined,
    error: enabled ? result?.error : undefined,
    loading: enabled && (pending || !result),
    retry: () => setAttempt((value) => value + 1),
  };
};

export const useCatalog = (
  endpoint: string | null,
  directory: ReturnType<typeof useKeylessDirectory>
) => {
  const sample = sampleResponse(endpoint);
  const data =
    endpoint && directory.data
      ? selectCatalog(directory.data, endpoint)
      : undefined;
  const missing = Boolean(
    endpoint &&
    /\/display\/[^/]+\/[^/]+$/u.test(
      new URL(endpoint, location.origin).pathname
    ) &&
    data &&
    !data.release
  );
  return {
    data: sample?.data ?? data,
    error:
      sample?.error ??
      (missing
        ? "This exact release is not in the current catalog."
        : directory.error),
    loading: !sample && Boolean(endpoint) && directory.loading,
    retry: directory.retry,
    status: sample?.status ?? (missing ? 404 : undefined),
  };
};

// Expiry can pass while the page stays open. Recompute on visibility changes too.
export const useCatalogStale = (data?: Catalog) => {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const update = () => setNow(Date.now());
    const remaining = (data?.expires_at ?? 0) * 1000 - Date.now();
    const timer =
      remaining > 0
        ? window.setTimeout(update, Math.min(remaining, 2_147_483_647))
        : undefined;
    update();
    document.addEventListener("visibilitychange", update);
    return () => {
      window.clearTimeout(timer);
      document.removeEventListener("visibilitychange", update);
    };
  }, [data?.expires_at]);
  return Boolean(
    data?.stale || (data?.expires_at && now >= data.expires_at * 1000)
  );
};
