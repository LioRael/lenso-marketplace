import { Button } from "@lenso/ui/button";
import { IconButton } from "@lenso/ui/icon-button";
import { TextField } from "@lenso/ui/text-field";
import * as stylex from "@stylexjs/stylex";
import { Search, X, Sun, Moon } from "lucide-react";
import { useEffect, useRef, useState } from "react";

import { useCatalog } from "./catalog";
import { styles as layout } from "./catalog-layout.stylex";
import { Detail, Filters, Guide, ReleaseList, State } from "./components";
import { controls } from "./controls";
import { identity } from "./model";
import {
  browseUrl,
  rootUrl,
  useClientNavigation,
  navigate,
  publisherUrl,
  useRoute,
} from "./navigation";
import { useSaved } from "./saved";

const initialTheme = () => {
  try {
    const stored = sessionStorage.getItem("lenso-marketplace-theme");
    if (stored) {
      return stored === "dark";
    }
  } catch {
    /* Optional storage. */
  }
  return true;
};
const catalogHeading = (route: ReturnType<typeof useRoute>) => {
  if (route.view === "saved") {
    return "Saved plugins";
  }
  if (route.q) {
    return "Search results";
  }
  return route.publisher || "Plugins for your workspace";
};
const useMarketplace = () => {
  useClientNavigation();
  const route = useRoute();
  const saved = useSaved(route.sample);
  const [query, setQuery] = useState(route.q);
  const [dark, setDark] = useState(initialTheme);
  const searchRef = useRef<HTMLElement>(null);
  const undoRef = useRef<HTMLButtonElement>(null);
  const previousSelection = useRef<string | null>(null);
  const selected = Boolean(route.id && route.version);
  useEffect(() => setQuery(route.q), [route.q]);
  useEffect(() => {
    const root = document.documentElement;
    root.classList.add("theme-switching");
    root.dataset.theme = dark ? "dark" : "light";
    void root.offsetHeight;
    const frame = requestAnimationFrame(() =>
      root.classList.remove("theme-switching")
    );
    try {
      sessionStorage.setItem(
        "lenso-marketplace-theme",
        dark ? "dark" : "light"
      );
    } catch {
      /* Optional storage. */
    }
    return () => {
      cancelAnimationFrame(frame);
      root.classList.remove("theme-switching");
    };
  }, [dark]);
  useEffect(() => {
    const shortcut = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        const current = new URLSearchParams(location.search);
        if (
          current.get("view") === "guide" ||
          current.get("view") === "publishers"
        ) {
          navigate(rootUrl());
        } else if (current.has("plugin")) {
          navigate(browseUrl());
        }
        requestAnimationFrame(() => searchRef.current?.focus());
      }
    };
    window.addEventListener("keydown", shortcut);
    return () => window.removeEventListener("keydown", shortcut);
  }, []);
  const queryParams = new URLSearchParams({
    limit: "30",
    offset: String(route.offset),
    q: route.q,
  });
  if (route.publisher) {
    queryParams.set("publisher", route.publisher);
  }
  if (route.license) {
    queryParams.set("license", route.license);
  }
  if (route.view === "saved") {
    queryParams.set("ids", saved.ids.join(","));
  }
  const api = route.sample
    ? "/sample-api/plugins"
    : "/api/marketplace/v1/plugins";
  const catalog = useCatalog(
    route.view === "guide" ? null : `${api}?${queryParams}`
  );
  const exact = useCatalog(
    selected
      ? `${api}/${encodeURIComponent(route.id ?? "")}/${encodeURIComponent(route.version ?? "")}`
      : null
  );
  const release = exact.data?.release;
  useEffect(() => {
    document.title = release
      ? `${release.title} ${release.version} · Lenso Marketplace`
      : "Lenso Plugin Marketplace";
  }, [release]);
  useEffect(() => {
    document.querySelector(".detail-pane")?.scrollTo(0, 0);
  }, [route.id, route.version]);
  useEffect(() => {
    if (selected) {
      previousSelection.current = `${route.id}@${route.version}`;
      if (release) {
        document
          .querySelector<HTMLElement>("[data-detail-title]")
          ?.focus({ preventScroll: true });
      }
    } else if (previousSelection.current) {
      document
        .querySelector<HTMLElement>(
          `[data-release="${CSS.escape(previousSelection.current)}"]`
        )
        ?.focus({ preventScroll: true });
      previousSelection.current = null;
    }
  }, [release, selected, route.id, route.version]);
  useEffect(() => {
    if (route.view === "saved" && saved.removed) {
      undoRef.current?.focus();
    }
  }, [saved.removed, route.view]);
  const removedKey = useRef<string | null>(null);
  useEffect(() => {
    if (saved.removed) {
      removedKey.current = identity(saved.removed);
      return;
    }
    if (!removedKey.current || selected) {
      return;
    }
    const target = document.querySelector<HTMLElement>(
      `[data-release="${CSS.escape(removedKey.current)}"]`
    );
    if (target) {
      target.focus({ preventScroll: true });
      removedKey.current = null;
    }
  }, [saved.removed, catalog.data, selected]);
  useEffect(() => {
    document.querySelector(".catalog-results")?.scrollTo(0, 0);
  }, [route.q, route.publisher, route.license, route.offset, route.view]);
  const releases = (catalog.data?.releases ?? []).filter(
    (item) => route.view !== "saved" || saved.ids.includes(identity(item))
  );
  const total = catalog.data?.total ?? releases.length;
  const filtered = Boolean(route.q || route.publisher || route.license);
  const heading = catalogHeading(route);
  let closeLabel = "All plugins";
  if (filtered) {
    closeLabel = "Search results";
  }
  if (route.view === "saved") {
    closeLabel = "Saved plugins";
  }
  const browse = route.view === "browse" || route.view === "saved";
  return {
    browse,
    catalog,
    closeLabel,
    dark,
    exact,
    filtered,
    heading,
    query,
    release,
    releases,
    route,
    saved,
    searchRef,
    selected,
    setDark,
    setQuery,
    total,
    undoRef,
  };
};
const Publishers = ({
  catalog,
}: {
  catalog: ReturnType<typeof useCatalog>;
}) => (
  <section className="publishers-page">
    <h1>Publishers</h1>
    {catalog.error ? (
      <State
        title="Catalog unavailable"
        action={
          <Button xstyle={controls.action} onClick={() => catalog.retry()}>
            Try again
          </Button>
        }
      >
        {catalog.error}
      </State>
    ) : (
      <div className="publisher-list">
        {catalog.data?.publishers?.map((name) => (
          <a href={publisherUrl(name)} key={name}>
            {name}
            <span>View plugins →</span>
          </a>
        ))}
      </div>
    )}
  </section>
);

const CatalogFooter = ({ sample }: { sample: boolean }) => (
  <footer>
    <span>
      {sample ? "Sample catalog · Design preview" : "Lenso Plugin Marketplace"}
    </span>
    <a href={sample ? "/" : "/?catalog=sample"}>
      {sample ? "View real catalog" : "Explore the sample catalog"}
    </a>
  </footer>
);

export const App = () => {
  const {
    browse,
    catalog,
    closeLabel,
    dark,
    exact,
    filtered,
    heading,
    query,
    release,
    releases,
    route,
    saved,
    searchRef,
    selected,
    setDark,
    setQuery,
    total,
    undoRef,
  } = useMarketplace();
  const clearSearch = () => {
    setQuery("");
    navigate(browseUrl({ offset: null, q: null }));
    searchRef.current?.focus();
  };
  const clear = (
    <Button
      xstyle={controls.action}
      variant="secondary"
      render={
        <a
          className="catalog-action-link"
          href={browseUrl({
            license: null,
            offset: null,
            publisher: null,
            q: null,
          })}
          aria-label="Clear all filters"
        />
      }
    >
      Clear all filters
    </Button>
  );
  const renderList = () => {
    if (catalog.loading && !catalog.data) {
      return <State title="Loading catalog">Loading published releases…</State>;
    }
    if (catalog.error) {
      return (
        <State
          title="Catalog unavailable"
          action={
            <Button xstyle={controls.action} onClick={() => catalog.retry()}>
              Try again
            </Button>
          }
        >
          {catalog.error}
        </State>
      );
    }
    if (!releases.length) {
      let title = "No published plugins";
      let description =
        "Try a different search or check back for published releases.";
      if (route.view === "saved") {
        description = saved.ids.length
          ? "Your saved releases are not currently listed. Browse the catalog for available plugins."
          : "Save a plugin from its detail page to find it here later.";
        title = saved.ids.length
          ? "Saved releases unavailable"
          : "No saved plugins yet";
      }
      if (filtered) {
        title = "No matching plugins";
      }
      if (route.offset) {
        return (
          <State
            title="No more plugins"
            action={
              <Button
                xstyle={controls.action}
                render={
                  <a
                    className="catalog-action-link"
                    href={browseUrl({
                      offset: String(Math.max(0, route.offset - 30)),
                    })}
                    aria-label="Previous page"
                  />
                }
              >
                Previous page
              </Button>
            }
          >
            You have reached the end of these results.
          </State>
        );
      }
      return (
        <State
          title={title}
          action={
            filtered ? (
              clear
            ) : (
              <Button
                xstyle={controls.action}
                render={
                  <a
                    className="catalog-action-link"
                    href={rootUrl()}
                    aria-label="Show all plugins"
                  />
                }
              >
                Show all plugins
              </Button>
            )
          }
        >
          {description}
        </State>
      );
    }
    return <ReleaseList releases={releases} route={route} saved={saved} />;
  };
  const renderDetail = () => {
    if (exact.loading && !release) {
      return <State title="Loading release">Loading release details…</State>;
    }
    if (exact.error) {
      return (
        <State
          title={
            exact.status === 404 ? "Release not found" : "Release unavailable"
          }
          action={
            <Button xstyle={controls.action} onClick={() => exact.retry()}>
              Try again
            </Button>
          }
        >
          {exact.error}
        </State>
      );
    }
    return release ? <Detail release={release} saved={saved} /> : null;
  };
  return (
    <div className="marketplace">
      <header className="app-header">
        <a href={rootUrl()} className="brand">
          Lenso <span>Marketplace</span>
        </a>
        <nav aria-label="Marketplace">
          <a
            href={rootUrl()}
            aria-current={route.view === "browse" ? "page" : undefined}
          >
            Browse
          </a>
          <a
            href={rootUrl({ view: "saved" })}
            aria-label={`Saved plugins ${saved.ids.length}`}
            aria-current={route.view === "saved" ? "page" : undefined}
          >
            Saved <span>{saved.ids.length}</span>
          </a>
          <a
            href={rootUrl({ view: "guide" })}
            aria-current={route.view === "guide" ? "page" : undefined}
          >
            Developers
          </a>
        </nav>
        <IconButton
          xstyle={controls.themeButton}
          title={dark ? "Switch to light theme" : "Switch to dark theme"}
          variant="ghost"
          onClick={() => setDark((value) => !value)}
          aria-label={`Switch to ${dark ? "light" : "dark"} theme`}
        >
          {dark ? (
            <Sun size={17} aria-hidden="true" />
          ) : (
            <Moon size={17} aria-hidden="true" />
          )}
        </IconButton>
      </header>
      <main>
        {browse && (
          <div className="browser" data-selected={selected || undefined}>
            <section
              className="catalog-pane"
              aria-label="Plugin catalog"
              hidden={selected}
            >
              <section
                {...stylex.props(layout.root, controls.catalog)}
                data-slot="catalog-layout"
              >
                <header
                  {...stylex.props(layout.header)}
                  data-slot="catalog-header"
                >
                  <div className="catalog-heading">
                    <h1
                      {...stylex.props(layout.title)}
                      data-slot="catalog-title"
                    >
                      {heading}
                    </h1>
                    <output>
                      {catalog.loading
                        ? "Loading…"
                        : `${total} ${total === 1 ? "release" : "releases"}`}
                    </output>
                  </div>
                  <form
                    className="search"
                    aria-label="Plugin search"
                    onSubmit={(event) => {
                      event.preventDefault();
                      navigate(browseUrl({ offset: null, q: query.trim() }));
                    }}
                  >
                    <TextField.Root xstyle={controls.field}>
                      <TextField.InputGroup xstyle={controls.searchGroup}>
                        <TextField.Leading>
                          <Search size={16} aria-hidden="true" />
                        </TextField.Leading>
                        <TextField.Control
                          xstyle={controls.searchText}
                          ref={searchRef}
                          aria-label="Search plugins"
                          placeholder={
                            route.view === "saved"
                              ? "Search saved plugins…"
                              : "Search plugins…"
                          }
                          value={query}
                          onValueChange={setQuery}
                          maxLength={256}
                          autoComplete="off"
                          spellCheck={false}
                          enterKeyHint="search"
                          aria-keyshortcuts="Control+k Meta+k"
                          onKeyDown={(event) => {
                            if (event.nativeEvent.isComposing) {
                              if (event.key === "Enter") {
                                event.preventDefault();
                              }
                              return;
                            }
                            if (event.key === "Escape") {
                              clearSearch();
                            }
                          }}
                        />
                        <TextField.Trailing>
                          <TextField.Clear
                            aria-label="Clear search"
                            hidden={!query}
                            onClear={clearSearch}
                          >
                            <X size={14} aria-hidden="true" />
                          </TextField.Clear>
                        </TextField.Trailing>
                      </TextField.InputGroup>
                    </TextField.Root>
                    <Button
                      xstyle={controls.searchButton}
                      type="submit"
                      variant="secondary"
                    >
                      Search
                    </Button>
                  </form>
                  <Filters route={route} catalog={catalog.data} />
                </header>
                <section
                  className="catalog-results"
                  aria-label="Search results"
                  aria-busy={catalog.loading}
                >
                  {renderList()}
                </section>
              </section>
              {releases.length > 0 &&
                (route.offset > 0 ||
                  route.offset + releases.length < total) && (
                  <nav className="pagination" aria-label="Result pages">
                    <Button
                      xstyle={controls.action}
                      variant="ghost"
                      disabled={route.offset === 0}
                      onClick={() =>
                        navigate(
                          browseUrl({
                            offset: String(Math.max(0, route.offset - 30)),
                          })
                        )
                      }
                    >
                      Previous
                    </Button>
                    <span>
                      {route.offset + 1}–{route.offset + releases.length} of{" "}
                      {total}
                    </span>
                    <Button
                      xstyle={controls.action}
                      variant="ghost"
                      disabled={route.offset + releases.length >= total}
                      onClick={() =>
                        navigate(
                          browseUrl({ offset: String(route.offset + 30) })
                        )
                      }
                    >
                      Next
                    </Button>
                  </nav>
                )}
            </section>
            <section
              className="detail-pane"
              hidden={!selected}
              aria-label="Release details"
              aria-busy={exact.loading}
            >
              {selected && (
                <a className="close-detail" href={browseUrl()}>
                  ← {closeLabel}
                </a>
              )}
              {renderDetail()}
            </section>
          </div>
        )}
        {route.view === "guide" && <Guide />}
        {route.view === "publishers" && <Publishers catalog={catalog} />}
      </main>
      <CatalogFooter sample={route.sample} />
      <div className="notice" hidden={!saved.message}>
        <output aria-live="polite">{saved.message}</output>
        {saved.removed && (
          <Button
            xstyle={controls.action}
            ref={undoRef}
            variant="secondary"
            onClick={() => saved.undo()}
          >
            Undo
          </Button>
        )}
        <Button
          xstyle={controls.action}
          variant="ghost"
          aria-label="Dismiss notification"
          onClick={() => {
            saved.dismiss();
            if (selected) {
              document
                .querySelector<HTMLElement>("[data-detail-title]")
                ?.focus();
            } else {
              searchRef.current?.focus();
            }
          }}
        >
          Dismiss
        </Button>
      </div>
    </div>
  );
};
