import { X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { isArrayOp, type DraftWhere } from "@/lib/queryDraft";
import { COMPARE_OPS, VALUE_TYPES } from "@/stores/queryStore";
import type { CompareOp } from "@/types";
import type { DraftValueType } from "@/lib/queryDraft";

export function WhereRow({
  where,
  onChange,
  onRemove,
}: {
  where: DraftWhere;
  onChange: (patch: Partial<DraftWhere>) => void;
  onRemove: () => void;
}) {
  const arrayOp = isArrayOp(where.op);
  const valueDisabled = where.valueType === "null";

  return (
    <div className="flex items-center gap-1.5">
      <Input
        value={where.field}
        onChange={(e) => onChange({ field: e.target.value })}
        placeholder="field (예: profile.age)"
        className="h-7 flex-1 text-xs"
        aria-label="필드"
      />
      <Select
        value={where.op}
        onValueChange={(v) => onChange({ op: v as CompareOp })}
      >
        <SelectTrigger className="h-7 w-[124px] text-xs" aria-label="연산자">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {COMPARE_OPS.map((op) => (
            <SelectItem key={op} value={op} className="text-xs">
              {op}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
      <Select
        value={where.valueType}
        onValueChange={(v) => onChange({ valueType: v as DraftValueType })}
      >
        <SelectTrigger className="h-7 w-[92px] text-xs" aria-label="값 타입">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {VALUE_TYPES.map((t) => (
            <SelectItem key={t} value={t} className="text-xs">
              {t}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
      <Input
        value={where.raw}
        onChange={(e) => onChange({ raw: e.target.value })}
        disabled={valueDisabled}
        placeholder={
          valueDisabled
            ? "—"
            : arrayOp
              ? "값1, 값2, 값3 (쉼표로 구분)"
              : where.valueType === "bool"
                ? "true / false"
                : "값"
        }
        className="h-7 flex-1 text-xs"
        aria-label="값"
      />
      <Button
        type="button"
        size="icon"
        variant="ghost"
        className="size-7 shrink-0"
        onClick={onRemove}
        aria-label="이 조건 삭제"
      >
        <X className="size-3.5" />
      </Button>
    </div>
  );
}
