import { useEffect, useState } from "react";

import type { Catalog } from "./model";
import { sampleResponse } from "./sample";

interface Result {
  endpoint: string;
  data?: Catalog;
  error?: string;
  status?: number;
}
export const useCatalog = (endpoint: string | null) => {
  const [result, setResult] = useState<Result>();
  const [pending, setPending] = useState(false);
  const [attempt, setAttempt] = useState(0);
  useEffect(() => {
    if (!endpoint || endpoint.startsWith("/sample-api/")) {
      return;
    }
    const controller = new AbortController();
    setPending(true);
    const load = async () => {
      try {
        const response = await fetch(endpoint, { signal: controller.signal });
        if (!response.ok) {
          const message =
            response.status === 404
              ? "This exact release is not in the catalog."
              : "The catalog could not be loaded. Try again.";
          if (!controller.signal.aborted) {
            setResult({ endpoint, error: message, status: response.status });
          }
          return;
        }
        const data: Catalog = await response.json();
        if (!controller.signal.aborted) {
          setResult({ data, endpoint });
        }
      } catch {
        if (!controller.signal.aborted) {
          setResult({
            endpoint,
            error: "Check your connection and try again.",
          });
        }
      } finally {
        if (!controller.signal.aborted) {
          setPending(false);
        }
      }
    };
    void load();
    return () => controller.abort();
  }, [endpoint, attempt]);
  const current =
    endpoint && result?.endpoint === endpoint ? result : undefined;
  const sample = sampleResponse(endpoint);
  return {
    data: sample?.data ?? current?.data,
    error: sample?.error ?? current?.error,
    loading: !sample && Boolean(endpoint) && (pending || !current),
    retry: () => setAttempt((value) => value + 1),
    status: sample?.status ?? current?.status,
  };
};
