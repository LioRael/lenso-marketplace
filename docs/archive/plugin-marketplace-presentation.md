# Marketplace release presentation

Catalog releases may include an optional `presentation` object. It belongs to the Marketplace release record, submitted and reviewed with the release through `Directory::submit`, `approve`, and `publish`. Console and Lenso UI do not own this metadata. No new upload API is introduced.

```json
{
  "presentation": {
    "icon_url": "https://publisher.example/echo/icon.png",
    "screenshots": [
      {
        "url": "https://publisher.example/echo/workspace.png",
        "caption": "Echo tool returning the supplied text"
      }
    ],
    "getting_started": "Call the Echo tool with a non-empty text argument.\n\nNo account is required."
  }
}
```

All three fields are optional. URLs must use HTTPS, include a host, contain no credentials or fragment, and fit in 2,048 UTF-8 bytes. A release may contain up to six screenshots with distinct URLs. Each screenshot requires a non-empty caption of at most 320 UTF-8 bytes, with no control characters. Getting-started text is limited to 16,384 UTF-8 bytes; only newline, carriage return and tab control characters are accepted. The existing `description` remains the overview text.

The signed snapshot covers URLs, captions and instructions. It does **not** authenticate externally hosted image bytes. The browser loads publisher images without a referrer; the server does not fetch them. Use stable, publicly accessible image URLs. Reviewers should inspect the referenced images before approving a submission. Images can still change at their origin after review.

The catalog renders descriptions and instructions as plain text, separated into paragraphs. It does not execute HTML or render Markdown. Missing or failed icons use a neutral package symbol. Failed screenshots retain their caption and a reserved preview area. Sample artwork stays confined to the explicit sample catalog; it is not substituted for real publisher assets.

## Compatibility and publication

Existing releases without `presentation` continue to deserialize and omit the field when serialized. Existing signed conformance vectors remain valid. Older strict readers reject the new populated field: upgrade catalog readers before publishing enriched snapshots. This is backward compatibility with old data, not forward compatibility for old readers.

Submission identity includes the complete release record. Presentation cannot be silently edited by resubmitting an already recorded plugin/version. Supply it with a new release and the matching verified bundle. Review approval covers that exact submission digest. Presentation does not grant installation authority or change the executable artifact identity.

## Validation

Catalog tests cover optional-field compatibility, signed round trips, tampered media references, URL restrictions and input bounds. The real-bundle acceptance test verifies submission, review, publication, restart and discovery with presentation metadata. Browser checks exercise publisher images, failed image fallbacks, literal instructions, narrow layouts and the existing catalog interaction suite.

The accepted compact typography, layout and Lenso UI controls remain the visual baseline. This change adds release content rather than changing that baseline.
