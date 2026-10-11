export function workspaceLocation(href: string, view: string) {
  const url = new URL(href);
  url.searchParams.set("view", view);
  if (!["overview", "todos", "goals", "review", "ledger-settings"].includes(view)) {
    for (const key of ["sessionId", "anchor", "detail"]) url.searchParams.delete(key);
  }
  return url;
}
