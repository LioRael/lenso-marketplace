export interface Release {
  plugin_id: string;
  version: string;
  title: string;
  summary: string;
  description?: string;
  presentation?: {
    icon_url?: string;
    screenshots?: { url: string; caption: string }[];
    getting_started?: string;
  };
  publisher_id: string;
  license: string;
  source_url: string;
  source_revision: string;
  availability: string;
  artifact: { digest: string; manifest_digest: string; size: number };
}
export interface Catalog {
  catalog_id: string;
  cached: boolean;
  stale?: boolean;
  expires_at?: number;
  total?: number;
  publishers?: string[];
  licenses?: string[];
  releases?: Release[];
  release?: Release;
}
export const identity = (release: Release) =>
  `${release.plugin_id}@${release.version}`;
