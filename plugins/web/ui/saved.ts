import { useEffect, useState } from "react";

import { identity } from "./model";
import type { Release } from "./model";

const read = (key: string) => {
  try {
    const value: unknown = JSON.parse(localStorage.getItem(key) ?? "[]");
    return Array.isArray(value)
      ? [
          ...new Set(
            value.filter(
              (item): item is string =>
                typeof item === "string" && item.length <= 256
            )
          ),
        ].slice(0, 50)
      : [];
  } catch {
    return [];
  }
};
export const useSaved = (sample = false) => {
  const key = sample
    ? "lenso-marketplace-sample-saved-v1"
    : "lenso-marketplace-saved-v1";
  const [collection, setCollection] = useState(() => ({ ids: read(key), key }));
  const ids = collection.key === key ? collection.ids : read(key);
  const [message, setMessage] = useState("");
  const [removed, setRemoved] = useState<Release>();
  useEffect(() => {
    setCollection({ ids: read(key), key });
    setMessage("");
    setRemoved(undefined);
    const sync = (event: StorageEvent) => {
      if (event.key === key || event.key === null) {
        setCollection({ ids: read(key), key });
      }
    };
    window.addEventListener("storage", sync);
    return () => window.removeEventListener("storage", sync);
  }, [key]);
  const update = (release: Release, save: boolean) => {
    const current = read(key);
    const releaseId = identity(release);
    const exists = current.includes(releaseId);
    if (save && !exists && current.length === 50) {
      setMessage("You can save up to 50 releases. Remove one first.");
      return;
    }
    const next = save
      ? [...new Set([...current, releaseId])]
      : current.filter((item) => item !== releaseId);
    try {
      localStorage.setItem(key, JSON.stringify(next));
      setCollection({ ids: next, key });
      setRemoved(save ? undefined : release);
      setMessage(
        save
          ? `${release.title} saved.`
          : `${release.title} removed from saved plugins.`
      );
    } catch {
      setRemoved(undefined);
      setMessage(
        "Browser storage is unavailable. Your saved collection was not changed."
      );
    }
  };
  return {
    dismiss: () => {
      setMessage("");
      setRemoved(undefined);
    },
    ids,
    message,
    removed,
    toggle: (release: Release) =>
      update(release, !read(key).includes(identity(release))),
    undo: () => {
      if (removed) {
        update(removed, true);
      }
    },
  };
};
export type Saved = ReturnType<typeof useSaved>;
