import { CalendarIcon } from "lucide-react";
import { zhCN } from "date-fns/locale";
import { format } from "date-fns";
import { Calendar } from "@/components/ui/calendar";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
export function DateTimePicker({
  value,
  onChange,
}: {
  value: Date;
  onChange: (date: Date) => void;
}) {
  return (
    <div className="date-time">
      <Popover>
        <PopoverTrigger asChild>
          <Button variant="outline" aria-label="选择导入日期">
            <CalendarIcon size={16} />
            {format(value, "yyyy 年 MM 月 dd 日")}
          </Button>
        </PopoverTrigger>
        <PopoverContent className="w-auto p-0" align="start">
          <Calendar
            locale={zhCN}
            mode="single"
            selected={value}
            defaultMonth={value}
            captionLayout="dropdown"
            startMonth={new Date(1900, 0)}
            endMonth={new Date(2100, 11)}
            onSelect={(date) => {
              if (date) {
                date.setHours(value.getHours(), value.getMinutes(), 0, 0);
                onChange(date);
              }
            }}
          />
        </PopoverContent>
      </Popover>
      <Input
        aria-label="导入时间"
        type="time"
        required
        value={format(value, "HH:mm")}
        onChange={(e) => {
          if (e.target.value) {
            const [h, m] = e.target.value.split(":").map(Number);
            const next = new Date(value);
            next.setHours(h, m, 0, 0);
            onChange(next);
          }
        }}
      />
    </div>
  );
}
