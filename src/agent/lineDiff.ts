/** Line-based LCS diff for small/medium files (bounded for UI performance). */

export type DiffOp = { kind: 'same' | 'add' | 'del'; text: string };

export type LineDiffResult = { ops: DiffOp[]; truncated: boolean };

export function lineDiff(oldText: string, newText: string, maxEach = 800): LineDiffResult {
  const a = oldText.replace(/\r\n/g, '\n').split('\n');
  const b = newText.replace(/\r\n/g, '\n').split('\n');
  const truncated = a.length > maxEach || b.length > maxEach;
  const aa = a.slice(0, maxEach);
  const bb = b.slice(0, maxEach);
  const n = aa.length;
  const m = bb.length;

  const dp: number[][] = Array.from({ length: n + 1 }, () => new Array(m + 1).fill(0));
  for (let i = 1; i <= n; i++) {
    for (let j = 1; j <= m; j++) {
      dp[i][j] = aa[i - 1] === bb[j - 1]
        ? dp[i - 1][j - 1] + 1
        : Math.max(dp[i - 1][j], dp[i][j - 1]);
    }
  }

  const opsRev: DiffOp[] = [];
  let i = n;
  let j = m;
  while (i > 0 || j > 0) {
    if (i > 0 && j > 0 && aa[i - 1] === bb[j - 1]) {
      opsRev.push({ kind: 'same', text: aa[i - 1] });
      i--;
      j--;
    } else if (j > 0 && (i === 0 || dp[i][j - 1] >= dp[i - 1][j])) {
      opsRev.push({ kind: 'add', text: bb[j - 1] });
      j--;
    } else if (i > 0) {
      opsRev.push({ kind: 'del', text: aa[i - 1] });
      i--;
    } else {
      break;
    }
  }
  opsRev.reverse();
  return { ops: opsRev, truncated };
}
