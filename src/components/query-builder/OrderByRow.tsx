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
import type { OrderBy } from "@/types";

export function OrderByRow({
  order,
  onChange,
  onRemove,
}: {
  order: OrderBy;
  onChange: (patch: Partial<OrderBy>) => void;
  onRemove: () => void;
}) {
  return (
    <div className="flex items-center gap-1.5">
      <Input
        value={order.field}
        onChange={(e) => onChange({ field: e.target.value })}
        placeholder="정렬 필드"
        className="h-7 flex-1 text-xs"
        aria-label="정렬 필드"
      />
      <Select
        value={order.direction}
        onValueChange={(v) => onChange({ direction: v as "asc" | "desc" })}
      >
        <SelectTrigger className="h-7 w-[88px] text-xs" aria-label="정렬 방향">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value="asc" className="text-xs">
            asc
          </SelectItem>
          <SelectItem value="desc" className="text-xs">
            desc
          </SelectItem>
        </SelectContent>
      </Select>
      <Button
        type="button"
        size="icon"
        variant="ghost"
        className="size-7 shrink-0"
        onClick={onRemove}
        aria-label="이 정렬 삭제"
      >
        <X className="size-3.5" />
      </Button>
    </div>
  );
}
