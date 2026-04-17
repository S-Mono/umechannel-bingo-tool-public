const BINGO_LINES: Array<{ key: string; numbers: number[] }> = [
  { key: "row-0", numbers: [1, 2, 3, 4, 5] },
  { key: "row-1", numbers: [6, 7, 8, 9, 10] },
  { key: "row-2", numbers: [11, 12, 13, 14, 15] },
  { key: "row-3", numbers: [16, 17, 18, 19, 20] },
  { key: "row-4", numbers: [21, 22, 23, 24, 25] },
  { key: "col-0", numbers: [1, 6, 11, 16, 21] },
  { key: "col-1", numbers: [2, 7, 12, 17, 22] },
  { key: "col-2", numbers: [3, 8, 13, 18, 23] },
  { key: "col-3", numbers: [4, 9, 14, 19, 24] },
  { key: "col-4", numbers: [5, 10, 15, 20, 25] },
  { key: "diag-main", numbers: [1, 7, 13, 19, 25] },
  { key: "diag-sub", numbers: [5, 9, 13, 17, 21] },
];

export const getCompletedBingoKeys = (hitNumbers: number[]): string[] => {
  const hitSet = new Set(hitNumbers);
  return BINGO_LINES.filter((line) => line.numbers.every((number) => hitSet.has(number))).map(
    (line) => line.key,
  );
};

export const getNewBingoKeys = (previousKeys: string[], hitNumbers: number[]): string[] => {
  const previousSet = new Set(previousKeys);
  return getCompletedBingoKeys(hitNumbers).filter((key) => !previousSet.has(key));
};