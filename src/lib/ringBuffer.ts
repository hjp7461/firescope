/** 끝에 push하되 길이를 max로 제한 (초과분은 앞에서 제거). 새 배열 반환. */
export function appendBounded<T>(buf: readonly T[], item: T, max: number): T[] {
  const next = [...buf, item];
  return next.length > max ? next.slice(next.length - max) : next;
}
