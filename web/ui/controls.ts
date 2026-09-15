import * as stylex from "@stylexjs/stylex";

export const controls = stylex.create({
  action: { fontSize: "var(--catalog-type-control)", lineHeight: 1.4 },
  catalog: { display: "flex", flex: 1, flexDirection: "column", minHeight: 0 },
  field: { maxWidth: "none", width: "100%" },
  freshness: { marginBottom: 16 },
  searchButton: { fontSize: "var(--catalog-type-control)", minHeight: 32 },
  searchGroup: { minHeight: 32 },
  searchText: { fontSize: "var(--catalog-type-control)" },
  state: { paddingBlock: 16, paddingInline: 0 },
  stateDescription: { fontSize: "var(--catalog-type-body)", lineHeight: 1.5 },
  stateTitle: { fontSize: "var(--catalog-type-name)", lineHeight: 1.4 },
  themeButton: { height: 32, width: 32 },
});
