import { zhCN } from "date-fns/locale";
import { Calendar } from "@/components/ui/calendar";

// The one month grid for picking a day, shared by 时刻's date heading and the import form so
// both look and behave alike: Chinese labels, month and year dropdowns, a fixed span of years.
export function DayCalendar({
  selected,
  onSelect,
}: {
  selected: Date;
  onSelect: (day: Date) => void;
}) {
  return (
    <Calendar
      locale={zhCN}
      mode="single"
      required
      selected={selected}
      defaultMonth={selected}
      captionLayout="dropdown"
      startMonth={new Date(1900, 0)}
      endMonth={new Date(2100, 11)}
      onSelect={(day) => {
        if (day) onSelect(day);
      }}
    />
  );
}
