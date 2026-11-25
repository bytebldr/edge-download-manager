import test from "node:test";
import assert from "node:assert/strict";
import { getMergedProgressSpans } from "../apps/desktop/src/utils/progress.ts";

test("getMergedProgressSpans: empty or 0 size returns empty array", () => {
  assert.deepStrictEqual(getMergedProgressSpans([], 0, 0, false), []);
});

test("getMergedProgressSpans: 100% complete produces one full span (0..100%)", () => {
  const workers100 = [
    { start: 0, end: 24, pos: 24 },
    { start: 25, end: 49, pos: 49 },
    { start: 50, end: 74, pos: 74 },
    { start: 75, end: 99, pos: 99 },
  ];
  const result100 = getMergedProgressSpans(workers100, 100, 100, true);
  assert.strictEqual(result100.length, 1);
  assert.strictEqual(result100[0].startPct, 0);
  assert.strictEqual(result100[0].widthPct, 100);
});

test("getMergedProgressSpans: unsorted and adjacent completed ranges merge without gaps", () => {
  const unsortedWorkers = [
    { start: 50, end: 75, pos: 75 },
    { start: 0, end: 25, pos: 25 },
    { start: 25, end: 50, pos: 50 },
    { start: 75, end: 100, pos: 100 },
  ];
  const resultMerged = getMergedProgressSpans(unsortedWorkers, 100, 100, false);
  assert.strictEqual(resultMerged.length, 1);
  assert.strictEqual(resultMerged[0].startPct, 0);
  assert.strictEqual(resultMerged[0].widthPct, 100);
});

test("getMergedProgressSpans: partial download preserves gaps between completed spans", () => {
  const partialWorkers = [
    { start: 0, end: 25, pos: 25 },   // 0..25 completed
    { start: 25, end: 50, pos: 35 },  // 25..35 completed (merges into 0..35)
    { start: 50, end: 75, pos: 50 },  // 0 bytes done (gap from 35 to 75)
    { start: 75, end: 100, pos: 85 }, // 75..85 completed
  ];
  const resultPartial = getMergedProgressSpans(partialWorkers, 100, 45, false);
  assert.strictEqual(resultPartial.length, 2);
  assert.strictEqual(resultPartial[0].startPct, 0);
  assert.strictEqual(resultPartial[0].widthPct, 35);
  assert.strictEqual(resultPartial[1].startPct, 75);
  assert.strictEqual(resultPartial[1].widthPct, 10);
});
