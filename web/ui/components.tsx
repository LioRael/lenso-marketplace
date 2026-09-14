/* oxlint-disable jsx-a11y/no-noninteractive-tabindex -- The screenshot gallery is a native overflow scroller; keyboard users need a focus target for arrow-key scrolling. */
import { Button } from "@lenso/ui/button";
import { ContentState } from "@lenso/ui/content-state";
import { Disclosure } from "@lenso/ui/disclosure";
import { Select } from "@lenso/ui/select";
import * as stylex from "@stylexjs/stylex";
import {
  Package,
  Bookmark,
  Check,
  ExternalLink,
  ChevronRight,
} from "lucide-react";
import { useState } from "react";
import type { ReactNode } from "react";

import { styles as layout } from "./catalog-layout.stylex";
import { controls } from "./controls";
import type { Catalog, Release } from "./model";
import { identity } from "./model";
import { browseUrl, navigate, publisherUrl, releaseUrl } from "./navigation";
import type { Route } from "./navigation";
import type { Saved } from "./saved";

const PublisherImage = ({
  src,
  caption = "",
  icon = false,
}: {
  src: string;
  caption?: string;
  icon?: boolean;
}) => {
  const [failed, setFailed] = useState(false);
  if (failed) {
    return icon ? (
      <Package size={32} aria-hidden="true" />
    ) : (
      <div className="preview-unavailable">Preview unavailable</div>
    );
  }
  return (
    <img
      src={src}
      alt={caption}
      className={icon ? "publisher-icon" : undefined}
      loading="lazy"
      decoding="async"
      referrerPolicy="no-referrer"
      onError={() => setFailed(true)}
    />
  );
};

// Artwork is explicit sample presentation, never signed catalog metadata.
const sampleArtwork = new Set([
  "projects",
  "observe",
  "git",
  "files",
  "notes",
  "echo",
]);
const PluginIcon = ({ release }: { release: Release }) => {
  if (release.presentation?.icon_url) {
    return (
      <PublisherImage
        key={release.presentation.icon_url}
        src={release.presentation.icon_url}
        icon
      />
    );
  }
  const slug = release.plugin_id.replace(/^sample\./u, "");
  if (release.availability === "sample" && sampleArtwork.has(slug)) {
    return (
      <img
        className="plugin-artwork"
        src={`/sample-assets/${slug}.png`}
        alt=""
        width={64}
        height={64}
        decoding="async"
      />
    );
  }
  return <Package size={32} strokeWidth={1.6} />;
};

export const State = ({
  title,
  children,
  action,
}: {
  title: string;
  children?: ReactNode;
  action?: ReactNode;
}) => (
  <ContentState.Root align="start" xstyle={controls.state}>
    <ContentState.Title as="h2" xstyle={controls.stateTitle}>
      {title}
    </ContentState.Title>
    <ContentState.Description xstyle={controls.stateDescription}>
      {children}
    </ContentState.Description>
    {action && <ContentState.Actions>{action}</ContentState.Actions>}
  </ContentState.Root>
);
export const SaveButton = ({
  release,
  saved,
  prominent = false,
}: {
  release: Release;
  saved: Saved;
  prominent?: boolean;
}) => {
  const active = saved.ids.includes(identity(release));
  return (
    <Button
      xstyle={controls.action}
      variant={prominent ? "primary" : "ghost"}
      aria-label={`${active ? "Unsave" : "Save"} ${release.title}`}
      aria-pressed={active}
      onClick={() => saved.toggle(release)}
    >
      {active ? (
        <Check size={15} aria-hidden="true" />
      ) : (
        <Bookmark size={15} aria-hidden="true" />
      )}
      {active ? "Saved" : "Save plugin"}
    </Button>
  );
};
export const Filter = ({
  label,
  value,
  values,
  field,
}: {
  label: string;
  value: string;
  values: string[];
  field: string;
}) => (
  <Select.Root
    value={value}
    onValueChange={(next) =>
      navigate(browseUrl({ [field]: String(next ?? ""), offset: null }))
    }
  >
    <Select.Trigger
      xstyle={controls.action}
      aria-label={label}
      title={value || label}
    >
      <Select.Value>{value || label}</Select.Value>
      <Select.Icon>⌄</Select.Icon>
    </Select.Trigger>
    <Select.Portal>
      <Select.Positioner>
        <Select.Popup>
          <Select.List>
            <Select.Item value="">
              <Select.ItemText>All {label.toLowerCase()}</Select.ItemText>
            </Select.Item>
            {[...new Set([...(value ? [value] : []), ...values])].map(
              (item) => (
                <Select.Item key={item} value={item}>
                  <Select.ItemText>{item}</Select.ItemText>
                </Select.Item>
              )
            )}
          </Select.List>
        </Select.Popup>
      </Select.Positioner>
    </Select.Portal>
  </Select.Root>
);

export const Filters = ({
  route,
  catalog,
}: {
  route: Route;
  catalog?: Catalog;
}) => {
  const applied = [
    ["q", "Search", route.q],
    ["publisher", "Publisher", route.publisher],
    ["license", "License", route.license],
  ];
  return (
    <>
      <div className="filters">
        {((catalog?.publishers?.length ?? 0) > 1 || route.publisher) && (
          <Filter
            label="Publishers"
            field="publisher"
            value={route.publisher}
            values={catalog?.publishers ?? []}
          />
        )}
        {((catalog?.licenses?.length ?? 0) > 1 || route.license) && (
          <Filter
            label="Licenses"
            field="license"
            value={route.license}
            values={catalog?.licenses ?? []}
          />
        )}
      </div>
      {applied.some((item) => item[2]) && (
        <nav className="applied-filters" aria-label="Active filters">
          {applied
            .filter((item) => item[2])
            .map(([field, label, value]) => (
              <a
                key={field}
                href={browseUrl({ [field]: null, offset: null })}
                aria-label={`Remove ${label.toLowerCase()} filter: ${value}`}
              >
                <span>
                  {label}: {value}
                </span>
                <span aria-hidden="true">×</span>
              </a>
            ))}
          <a
            href={browseUrl({
              license: null,
              offset: null,
              publisher: null,
              q: null,
            })}
          >
            Clear filters
          </a>
        </nav>
      )}
    </>
  );
};
export const ReleaseList = ({
  releases,
  route,
  saved,
}: {
  releases: Release[];
  route: Route;
  saved: Saved;
}) => (
  <div {...stylex.props(layout.grid)} data-slot="catalog-grid">
    {releases.map((release) => (
      <article
        {...stylex.props(layout.item)}
        data-slot="catalog-item"
        key={identity(release)}
      >
        <div
          {...stylex.props(layout.media)}
          data-slot="catalog-media"
          data-plugin={
            release.availability === "sample" ? release.plugin_id : undefined
          }
          aria-hidden="true"
        >
          <PluginIcon release={release} />
        </div>
        <div {...stylex.props(layout.content)} data-slot="catalog-content">
          <a
            {...stylex.props(layout.link)}
            data-slot="catalog-link"
            aria-label={`${release.title} ${release.version}`}
            data-release={identity(release)}
            href={releaseUrl(release)}
          >
            <h2 {...stylex.props(layout.name)} data-slot="catalog-name">
              {release.title}
            </h2>
            <p {...stylex.props(layout.summary)} data-slot="catalog-summary">
              {release.summary}
            </p>
          </a>
          {release.availability !== "sample" && (
            <div {...stylex.props(layout.meta)} data-slot="catalog-meta">
              <a href={publisherUrl(release.publisher_id)}>
                {release.publisher_id}
              </a>
              <span>{release.version}</span>
            </div>
          )}
        </div>
        <div {...stylex.props(layout.itemActions)} data-slot="catalog-actions">
          <span className="open-release" aria-hidden="true">
            <ChevronRight size={16} />
          </span>
          {route.view === "saved" && (
            <SaveButton release={release} saved={saved} />
          )}
        </div>
      </article>
    ))}
  </div>
);

const availabilityCaption = (availability: string) => {
  if (availability === "listed") {
    return null;
  }
  if (availability === "sample") {
    return "Preview — installation unavailable";
  }
  return `This release is ${availability}`;
};

export const Detail = ({
  release,
  saved,
}: {
  release: Release;
  saved: Saved;
}) => (
  <article {...stylex.props(layout.root)} data-slot="detail-layout">
    <header {...stylex.props(layout.detailHeader)} data-slot="detail-header">
      <div
        {...stylex.props(layout.detailMedia)}
        data-slot="detail-media"
        data-plugin={
          release.availability === "sample" ? release.plugin_id : undefined
        }
        aria-hidden="true"
      >
        <PluginIcon release={release} />
      </div>
      <div {...stylex.props(layout.content)} data-slot="detail-identity">
        <h1
          {...stylex.props(layout.title)}
          data-slot="detail-title"
          tabIndex={-1}
          data-detail-title
        >
          {release.title}
        </h1>
        <p {...stylex.props(layout.summary)} data-slot="detail-summary">
          {release.summary}
        </p>
        <div {...stylex.props(layout.meta)} data-slot="detail-meta">
          <a href={publisherUrl(release.publisher_id)}>
            {release.publisher_id}
          </a>
          <span>Version {release.version}</span>
        </div>
      </div>
      <div {...stylex.props(layout.detailActions)} data-slot="detail-actions">
        <SaveButton release={release} saved={saved} prominent />
        <div {...stylex.props(layout.meta)} data-slot="detail-meta">
          {availabilityCaption(release.availability)}
        </div>
      </div>
    </header>
    {release.availability === "sample" &&
      release.plugin_id === "sample.projects" && (
        <figure className="plugin-preview">
          <img
            src="/sample-assets/projects-preview.png"
            alt="Illustrative Projects workspace showing a task list and project context."
            width={1774}
            height={887}
            decoding="async"
          />
          <figcaption>Example workspace · Design illustration</figcaption>
        </figure>
      )}
    {!!release.presentation?.screenshots?.length && (
      <section
        className="publisher-gallery"
        aria-label="Plugin screenshots"
        tabIndex={0}
      >
        {release.presentation?.screenshots?.map((screenshot) => (
          <figure
            className="plugin-preview publisher-preview"
            key={screenshot.url}
          >
            <PublisherImage
              key={screenshot.url}
              src={screenshot.url}
              caption={screenshot.caption}
            />
            <figcaption>{screenshot.caption}</figcaption>
          </figure>
        ))}
      </section>
    )}
    <div
      {...stylex.props(
        layout.body,
        release.availability === "sample" && layout.compactBody
      )}
      data-slot="detail-body"
    >
      <section
        {...stylex.props(layout.main)}
        data-slot="detail-main"
        aria-label="Overview"
      >
        <h2
          {...stylex.props(layout.sectionTitle)}
          data-slot="detail-section-title"
        >
          Overview
        </h2>
        {(release.description || release.summary)
          .split(/\n\s*\n/u)
          .map((paragraph, index) => (
            <p
              {...stylex.props(layout.paragraph)}
              data-slot="detail-paragraph"
              key={`${index}-${paragraph.slice(0, 32)}`}
            >
              {paragraph}
            </p>
          ))}
        {release.presentation?.getting_started && (
          <section className="getting-started" aria-label="Getting started">
            <h3 {...stylex.props(layout.sectionTitle)}>Getting started</h3>
            {release.presentation.getting_started
              .split(/\n\s*\n/u)
              .map((paragraph, index) => (
                <p {...stylex.props(layout.paragraph)} key={index}>
                  {paragraph}
                </p>
              ))}
          </section>
        )}
      </section>
      <aside
        {...stylex.props(
          layout.aside,
          release.availability === "sample" && layout.compactAside
        )}
        data-slot="detail-aside"
        aria-label="Release information"
      >
        <h2
          {...stylex.props(layout.sectionTitle)}
          data-slot="detail-section-title"
        >
          Release information
        </h2>
        <dl className="facts">
          <div>
            <dt>License</dt>
            <dd>{release.license}</dd>
          </div>
          {release.availability !== "sample" && (
            <div>
              <dt>Package size</dt>
              <dd>
                {(release.artifact.size / 1024).toLocaleString(undefined, {
                  maximumFractionDigits: 1,
                })}{" "}
                KiB
              </dd>
            </div>
          )}
          {release.availability !== "sample" && (
            <div className="plugin-id">
              <dt>Plugin ID</dt>
              <dd>{release.plugin_id}</dd>
            </div>
          )}
        </dl>
        <div className="resource-links">
          {!/\.(test|invalid|localhost)$/u.test(
            new URL(release.source_url).hostname
          ) && (
            <a
              href={release.source_url}
              target="_blank"
              rel="noopener noreferrer"
              aria-label="View source (opens in a new tab)"
            >
              View source <ExternalLink size={14} aria-hidden="true" />
            </a>
          )}
          <a href={publisherUrl(release.publisher_id)}>
            More from this publisher
          </a>
        </div>
        {release.availability !== "sample" && (
          <Disclosure.Root>
            <Disclosure.Item>
              <Disclosure.Header>
                <Disclosure.Trigger>
                  Release integrity <Disclosure.Icon>⌄</Disclosure.Icon>
                </Disclosure.Trigger>
              </Disclosure.Header>
              <Disclosure.Panel>
                <p className="integrity-note">
                  The catalog signature verifies this release record. It is not
                  a security review of the plugin.
                </p>
                <dl className="integrity-facts">
                  {[
                    ["Archive SHA-256", release.artifact.digest],
                    ["Manifest SHA-256", release.artifact.manifest_digest],
                    ["Source revision", release.source_revision],
                  ].map(([label, value]) => (
                    <div key={label}>
                      <dt>{label}</dt>
                      <dd className="code">{value}</dd>
                    </div>
                  ))}
                </dl>
              </Disclosure.Panel>
            </Disclosure.Item>
          </Disclosure.Root>
        )}
      </aside>
    </div>
  </article>
);

export const Guide = () => (
  <article className="guide">
    <h1>Developer guide</h1>
    <p>Check and package a plugin before preparing a release for review.</p>
    <h2>Check the plugin</h2>
    <code>lenso plugin check --repo-root ./my-plugin</code>
    <h2>Create a release bundle</h2>
    <code>
      lenso plugin pack --repo-root ./my-plugin --output ./plugin.lenso-plugin
    </code>
    <h2>Prepare for review</h2>
    <p>
      Include the publisher, license, source revision and bundle identity.
      Publication requires namespace ownership and reviewer approval.
    </p>
    <p className="muted">Online submissions are not open in this preview.</p>
  </article>
);
