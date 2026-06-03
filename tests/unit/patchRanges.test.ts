import { parsePatchTouchedRanges } from '../../src/main/analysis/patchRanges';

describe('patchRanges', () => {
  it('parses unified patch hunks into touched ranges', () => {
    const ranges = parsePatchTouchedRanges(`@@ -10,2 +10,3 @@
-old
+new
+extra
 context`);

    expect(ranges).toEqual([{ oldStart: 10, oldEnd: 11, newStart: 10, newEnd: 12 }]);
  });
});
