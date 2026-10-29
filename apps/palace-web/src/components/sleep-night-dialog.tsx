import { useState, type FormEvent } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { ApiError, api, apiData, refreshSleep } from "@/lib/api";
import { monthDay } from "@/lib/day";
import {
  clock,
  longDuration,
  minutesBetween,
  readNight,
  type Night,
} from "@/lib/sleep";
import { ErrorState } from "./error-state";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { Label } from "./ui/label";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "./ui/dialog";

// What the dialog is open for: recording this morning's night from the day's menu, filling in a
// day the history shows empty, or correcting a recorded night.
export type NightTarget =
  | { kind: "record"; day: Date }
  | { kind: "fill"; day: Date }
  | { kind: "edit"; day: Date; night: Night };

const WEEKDAYS = ["日", "一", "二", "三", "四", "五", "六"];

function titleOf(target: NightTarget): string {
  const day = monthDay(target.day);
  switch (target.kind) {
    case "record":
      return "记录睡眠";
    case "fill":
      return `补记 ${day}`;
    case "edit":
      return `${day} 周${WEEKDAYS[target.day.getDay()]}`;
  }
}

// One form for every way a night is written: two clock times, the night's length worked out as
// they are typed, and the reason when they do not add up. Correcting a night can also delete it,
// which asks once more.
export function SleepNightDialog({
  target,
  usual,
  onClose,
}: {
  target: NightTarget;
  // The bedtime and waking a new night starts from, so most entries only need a nudge.
  usual: [string, string];
  onClose: () => void;
}) {
  const client = useQueryClient();
  const [bed, setBed] = useState(() =>
    target.kind === "edit" ? clock(target.night.started_at) : usual[0],
  );
  const [wake, setWake] = useState(() =>
    target.kind === "edit" ? clock(target.night.ended_at) : usual[1],
  );
  const [confirmingDelete, setConfirmingDelete] = useState(false);
  const reading = readNight(target.day, bed, wake, new Date());

  const save = useMutation({
    mutationFn: async () => {
      if (reading.kind === "invalid") throw new Error(reading.reason);
      const body = {
        started_at: reading.started_at,
        ended_at: reading.ended_at,
      };
      try {
        return target.kind === "edit"
          ? await apiData(
              api.PUT("/api/sleep/nights/{id}", {
                params: { path: { id: target.night.id } },
                body,
              }),
            )
          : await apiData(api.POST("/api/sleep/nights", { body }));
      } catch (error) {
        // The one conflict a night can meet is another night over the same hours.
        if (error instanceof ApiError && error.status === 409)
          throw new ApiError(409, "和已经记下的一晚重叠了。");
        throw error;
      }
    },
    onSuccess: async () => {
      await refreshSleep(client);
      onClose();
    },
  });
  const deletion = useMutation({
    mutationFn: () => {
      if (target.kind !== "edit") throw new Error("没有可删除的一晚。");
      return apiData(
        api.DELETE("/api/sleep/nights/{id}", {
          params: { path: { id: target.night.id } },
        }),
      );
    },
    onSuccess: async () => {
      await refreshSleep(client);
      onClose();
    },
  });
  const busy = save.isPending || deletion.isPending;
  const typed = bed.trim() !== "" && wake.trim() !== "";
  const invalid = reading.kind === "invalid" && typed ? reading : null;

  return (
    <Dialog
      open
      onOpenChange={(open) => {
        if (!open && !busy) onClose();
      }}
    >
      <DialogContent className="sleep-dialog">
        <DialogHeader>
          <DialogTitle>{titleOf(target)}</DialogTitle>
        </DialogHeader>
        <form
          onSubmit={(event: FormEvent) => {
            event.preventDefault();
            save.mutate();
          }}
        >
          <fieldset disabled={busy} className="sleep-fields">
            <div>
              <Label htmlFor="sleep-bed">入睡</Label>
              <Input
                id="sleep-bed"
                inputMode="numeric"
                autoComplete="off"
                placeholder="23:30"
                value={bed}
                aria-invalid={invalid?.field === "bed"}
                onChange={(event) => setBed(event.target.value)}
              />
            </div>
            <div>
              <Label htmlFor="sleep-wake">醒来</Label>
              <Input
                id="sleep-wake"
                inputMode="numeric"
                autoComplete="off"
                placeholder="07:00"
                value={wake}
                aria-invalid={invalid?.field === "wake"}
                onChange={(event) => setWake(event.target.value)}
              />
            </div>
          </fieldset>
          <p
            className={invalid ? "sleep-length invalid" : "sleep-length"}
            aria-live="polite"
          >
            {reading.kind === "night"
              ? longDuration(
                  minutesBetween(reading.started_at, reading.ended_at),
                )
              : invalid?.reason}
          </p>
          {save.isError && <ErrorState error={save.error} />}
          {deletion.isError && <ErrorState error={deletion.error} />}
          <DialogFooter className="sleep-actions">
            {target.kind === "edit" &&
              (confirmingDelete ? (
                <Button
                  type="button"
                  variant="outline"
                  className="sleep-delete confirming"
                  disabled={busy}
                  onClick={() => deletion.mutate()}
                >
                  {deletion.isPending ? "正在删除…" : "确认删除"}
                </Button>
              ) : (
                <Button
                  type="button"
                  variant="ghost"
                  className="sleep-delete"
                  disabled={busy}
                  onClick={() => setConfirmingDelete(true)}
                >
                  删除这一晚
                </Button>
              ))}
            <span className="spacer" />
            <Button
              type="button"
              variant="ghost"
              disabled={busy}
              onClick={onClose}
            >
              取消
            </Button>
            <Button type="submit" disabled={busy || reading.kind === "invalid"}>
              {save.isPending ? "正在保存…" : "保存"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
