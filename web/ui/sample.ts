import type { Catalog, Release } from "./model";

// Explicit presentation data. Never passed to the signed catalog or installer.
const entries = [
  [
    "projects",
    "Projects",
    "Organize tasks and project context",
    "Keep tasks, notes and project context together. Related work can stay in one place.\n\nThis is a visual example of a project workflow, not an installed plugin.",
  ],
  [
    "observe",
    "Observe",
    "Inspect traces and follow execution",
    "Follow an execution from request to result. This example represents a place to inspect traces and understand where a run needs attention.",
  ],
  [
    "git",
    "Git Tools",
    "Review changes and repository history",
    "Browse changes and review repository history in the context of your work. This is a sample catalog entry for the visual preview.",
  ],
  [
    "files",
    "File Search",
    "Find files across a workspace",
    "Find a file by name and keep the surrounding project context close. This is a sample catalog entry for the visual preview.",
  ],
  [
    "notes",
    "Notes",
    "Keep project notes close to your work",
    "Capture decisions and working notes alongside your project. This is a sample catalog entry for the visual preview.",
  ],
  [
    "echo",
    "Echo",
    "Test a plugin connection",
    "Send a string and read it back to inspect a plugin round trip. This is a sample catalog entry for the visual preview.",
  ],
];
export const sampleReleases: Release[] = entries.map(
  ([slug, title, summary, description]) => ({
    artifact: { digest: "", manifest_digest: "", size: 0 },
    availability: "sample",
    description,
    license: "Not specified",
    plugin_id: `sample.${slug}`,
    publisher_id: "Lenso · Sample publisher",
    source_revision: "",
    source_url: "https://example.invalid",
    summary,
    title,
    version: "0.1.0",
  })
);
export const sampleResponse = (
  endpoint: string | null
): { data?: Catalog; error?: string; status?: number } | undefined => {
  if (!endpoint?.startsWith("/sample-api/")) {
    return undefined;
  }
  const url = new URL(endpoint, location.origin);
  const params = url.searchParams;
  const data: Catalog = {
    cached: false,
    catalog_id: "design-sample",
    licenses: ["Not specified"],
    publishers: ["Lenso · Sample publisher"],
  };
  const path = url.pathname.split("/");
  if (path.length > 3) {
    const release = sampleReleases.find(
      (item) =>
        item.plugin_id === decodeURIComponent(path[3]) &&
        item.version === decodeURIComponent(path[4])
    );
    return release
      ? { data: { ...data, release } }
      : { error: "This sample release does not exist.", status: 404 };
  }
  const query = (params.get("q") ?? "").toLowerCase();
  const matches = sampleReleases.filter((item) => {
    const text = `${item.title} ${item.summary}`.toLowerCase();
    return (
      text.includes(query) &&
      (!params.has("ids") ||
        params
          .get("ids")
          ?.split(",")
          .includes(`${item.plugin_id}@${item.version}`)) &&
      (!params.get("publisher") ||
        params.get("publisher") === item.publisher_id) &&
      (!params.get("license") || params.get("license") === item.license)
    );
  });
  const offset = Number(params.get("offset") ?? 0);
  return {
    data: {
      ...data,
      releases: matches.slice(offset, offset + 30),
      total: matches.length,
    },
  };
};
