# Catalog presentation review

## Ownership and intent

Catalog consumes the existing Lenso UI Button, IconButton, TextField, Select,
Disclosure and ContentState, with existing theme and semantic tokens. No library
component, Recipe or registry change is required. Business state, bookmarks,
routing and page composition remain Catalog-owned.

The first selected concept informs product presentation: recognizable plugin
identities, a two-column collection, and visual context before detail prose.
The earlier text-and-line-icon treatment removed this identity. The current
sample presentation restores six existing illustrative plugin images and an
explicitly captioned Projects workspace image. These are Catalog assets, never
signed catalog metadata or proof of an installed application. Real releases keep
the neutral fallback until their contract provides artwork.

## Composition

- Existing Lenso surfaces unify the header, browsing area and footer.
- A 28 px heading leads the search toolbar. Plugin rows pair 64 px artwork with
  16 px names and 14 px descriptions, with a 128 px minimum row height.
- Whole-row links preserve native navigation and full-row keyboard outlines.
  Publisher and Save actions remain independent. The scroll gutter reserves the
  outline outset, including the first row.
- Hover uses the existing interactive-hover token. Identity artwork stays stable;
  the trailing arrow changes text color without a second background.
- Projects sample detail includes a responsive workspace illustration, captioned
  “Example workspace · Design illustration”. Other sample details omit media.
- Search/filter grouping, empty-state typography, action contrast, reduced-motion
  behavior and theme switching retain the preceding fixes.

## Verification and limits

Frontend production build, embedded Host build, Rust formatting and focused lint
passed. Browser regression covers signed Echo reads, filtering, error recovery,
stale responses, save/undo, history, native link hit targets, focus visibility,
action contrast, long lists and narrow layouts. Asset requests are allowlisted;
unknown asset names return 404. Images are served separately from the script.

Directly inspected: 845 × 805 catalog in light and dark, desktop Projects detail,
and the 390 × 844 dark detail capture. Sample content and real catalog data remain
separate. Preview: http://127.0.0.1:63729/?catalog=sample.

Illustrations are existing full-resolution PNG preview assets; production image
variants and a publisher-artwork contract are not delivered by this visual pass.
RTL and 200% zoom were not verified. No publish or library modification occurred.

## Header refinement

The accepted catalog imagery and body layout are preserved. Desktop header now
groups brand/context on the leading edge and navigation/theme controls on the
trailing edge. Marketplace context uses secondary text and a quieter separator.
Navigation links have consistent 32 px targets, neutral hover surfaces and stable
font weight; a 2 px text-aligned neutral underline marks the current section.
Theme control uses the existing IconButton with a 32 px supported layout override
and an explicit title. Narrow layouts retain a second navigation row with labels
aligned to the content gutter. Inspected desktop dark/light, keyboard focus and
390 px dark screenshot. Frontend/Host builds, lint and browser regression passed.

## Typography pass

Catalog now defines a single role-based type scale: title 28 px, section 20 px,
release name 18 px, body/summary 15 px, control 14 px and caption 13 px. The Lenso
font family and existing controls are preserved; public xstyle props align the
consumer's action/search/select text with the control role. Header wordmark keeps
its 22 px brand treatment. Captions, versions, counts, code and footer no longer
fall to 11–12 px. Headings use a tighter unitless line height and balanced wrap;
prose/descriptions use 1.5 line height with pretty wrapping and bounded measure.
Developer guide and empty states use the same roles. Raster illustration text
remains part of the illustrative image and is not an application text surface.

Inspected dark desktop list/detail, light detail prose, and the 390 px dark
detail screenshot. Frontend/Host build, focused lint and the existing long-text,
responsive, action contrast, focus and navigation regression all passed.

## Compact density correction

The user found the expanded scale and spacing too large. Current roles supersede
those above: 24 px title, 16 px section, 15 px name, 13 px body/control and 12 px
caption. Wordmark is 20 px. Header is 56 px, search controls 32 px, artwork 56 px,
and release rows 96 px minimum. Desktop top inset is 24 px; header/list gaps,
detail section padding, preview/prose spacing and empty-state padding are reduced
together. Existing imagery, whole-row hit targets and focus gutters remain.
Frontend/Host builds, lint and browser regression passed; desktop list/detail
and the 390 px detail capture were visually inspected.
