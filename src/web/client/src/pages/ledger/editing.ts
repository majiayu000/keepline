import type { WorkItem } from "../../types/work-item";
import type { Anchors } from "../../../../../domain/ledger/types";

/** Keep evidence identities attached to unchanged criteria through insertion/reordering. */
export function reconcileChecklist(text: string, original: NonNullable<WorkItem["acceptance"]> = []) {
  const remaining = [...original];
  return text.split("\n").map(line => line.trim()).filter(Boolean).map(text => {
    const index = remaining.findIndex(criterion => criterion.text.trim() === text);
    const previous = index < 0 ? undefined : remaining.splice(index, 1)[0];
    return previous ? { ...previous, text } : { id: crypto.randomUUID(), text, completed: false };
  });
}

export function cleanRequirementAnchors(anchors: Anchors): Anchors {
  const lines = (values: string[]) => values.map(value => value.trim()).filter(Boolean);
  return { ...anchors, paths: lines(anchors.paths), commands: lines(anchors.commands), keywords: lines(anchors.keywords) };
}

export function clampOverviewHours(hours: number, retentionDays: number) {
  const maximum = retentionDays * 24;
  return Math.min(Number.isSafeInteger(hours) && hours >= 1 ? hours : 24, maximum);
}
