import { useState, type FormEvent, type KeyboardEvent } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  activitiesOptions,
  activityTypesOptions,
  api,
  apiData,
  refreshActivities,
} from "@/lib/api";
import {
  defaultStart,
  readActivity,
  typesByRecency,
  usualMinutes,
  type Activity,
  type ActivityType,
} from "@/lib/activities";
import { addDays, dayOf, dayParam, dayRange, monthDay, today } from "@/lib/day";
import { clock } from "@/lib/sleep";
import { ErrorState } from "./error-state";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { Label } from "./ui/label";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "./ui/dialog";

// What the dialog is open for: recording on a day, or correcting one already recorded.
export type ActivityTarget =
  { kind: "record"; day: Date } | { kind: "edit"; activity: Activity };

const WEEKDAYS = ["日", "一", "二", "三", "四", "五", "六"];

// The last 30 days of activities, which the dialog reads the usual lengths and recent types from.
export function recentActivitiesOptions() {
  const last = today();
  const first = addDays(last, -29);
  return activitiesOptions(dayParam(first), dayParam(last), {
    start: dayRange(first).start,
    end: dayRange(last).end,
  });
}

/**
 * 记录工夫 and its correction: a type picked from a row of words, the most recent first, with
 * a new one added in place; a start, a length in minutes or h:mm, and an optional note. The span
 * is worked out as it is typed, and the reason shown when it does not add up.
 */
export function ActivityDialog({
  target,
  onClose,
}: {
  target: ActivityTarget;
  onClose: () => void;
}) {
  const client = useQueryClient();
  const types = useQuery(activityTypesOptions());
  const recent = useQuery(recentActivitiesOptions());
  const day =
    target.kind === "edit" ? dayOf(target.activity.started_at) : target.day;
  const [typeId, setTypeId] = useState<string | null>(
    target.kind === "edit" ? target.activity.type_id : null,
  );
  const [start, setStart] = useState(
    target.kind === "edit" ? clock(target.activity.started_at) : null,
  );
  const [length, setLength] = useState(
    target.kind === "edit" ? String(target.activity.minutes) : null,
  );
  // A length typed by hand stays when another type is picked.
  const [lengthTyped, setLengthTyped] = useState(target.kind === "edit");
  const [note, setNote] = useState(
    target.kind === "edit" ? (target.activity.note ?? "") : "",
  );
  const [naming, setNaming] = useState<string | null>(null);
  const [confirmingDelete, setConfirmingDelete] = useState(false);

  const ordered = typesByRecency(types.data ?? [], recent.data ?? []);
  // A new entry starts with the most recent type and its usual length, once both are known.
  const chosen = typeId ?? ordered[0]?.id ?? null;
  const usual = chosen ? usualMinutes(recent.data ?? [], chosen) : 30;
  const lengthText = length ?? String(usual);
  const startText =
    start ?? clock(defaultStart(day, usual, new Date()).getTime());
  const reading = readActivity(day, startText, lengthText, new Date());

  const addType = useMutation({
    mutationFn: (name: string) =>
      apiData(api.POST("/api/activity-types", { body: { name } })),
    onSuccess: (added) => {
      client.setQueryData<ActivityType[]>(
        activityTypesOptions().queryKey,
        (current) =>
          current?.some((type) => type.id === added.id)
            ? current
            : [...(current ?? []), added],
      );
      pick(added.id);
      setNaming(null);
    },
  });
  const save = useMutation({
    mutationFn: () => {
      if (reading.kind === "invalid") throw new Error(reading.reason);
      if (!chosen) throw new Error("先选一个类型。");
      const body = {
        type_id: chosen,
        started_at: reading.started_at,
        minutes: reading.minutes,
        note: note.trim() || null,
      };
      return target.kind === "edit"
        ? apiData(
            api.PUT("/api/activities/{id}", {
              params: { path: { id: target.activity.id } },
              body,
            }),
          )
        : apiData(api.POST("/api/activities", { body }));
    },
    onSuccess: async () => {
      await refreshActivities(client);
      onClose();
    },
  });
  const deletion = useMutation({
    mutationFn: () => {
      if (target.kind !== "edit") throw new Error("没有可删除的工夫。");
      return apiData(
        api.DELETE("/api/activities/{id}", {
          params: { path: { id: target.activity.id } },
        }),
      );
    },
    onSuccess: async () => {
      await refreshActivities(client);
      onClose();
    },
  });
  const busy = save.isPending || deletion.isPending;
  const invalid = reading.kind === "invalid" ? reading : null;

  function pick(id: string) {
    setTypeId(id);
    if (!lengthTyped) setLength(null);
  }

  function nameKey(event: KeyboardEvent<HTMLInputElement>) {
    if (event.key === "Enter") {
      // Adds the type without submitting the whole form.
      event.preventDefault();
      const name = naming?.trim();
      if (name) addType.mutate(name);
    } else if (event.key === "Escape") {
      // Leaves the new name, not the dialog.
      event.stopPropagation();
      setNaming(null);
    }
  }

  const title =
    target.kind === "edit"
      ? `${monthDay(day)} 周${WEEKDAYS[day.getDay()]} · 工夫`
      : dayParam(day) === dayParam(today())
        ? "记录工夫"
        : `补记 ${monthDay(day)} 的工夫`;

  return (
    <Dialog
      open
      onOpenChange={(open) => {
        if (!open && !busy) onClose();
      }}
    >
      <DialogContent
        className="activity-dialog"
        // Most entries only nudge the length, so that is where typing starts.
        onOpenAutoFocus={(event) => {
          event.preventDefault();
          document.getElementById("activity-length")?.focus();
        }}
      >
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
          <DialogDescription className="sr-only">
            选择类型，填写开始时间、时长和备注
          </DialogDescription>
        </DialogHeader>
        <form
          onSubmit={(event: FormEvent) => {
            event.preventDefault();
            save.mutate();
          }}
        >
          <fieldset disabled={busy} className="activity-fields">
            <div className="activity-types" role="group" aria-label="类型">
              {ordered.map((type) => (
                <button
                  key={type.id}
                  type="button"
                  aria-pressed={type.id === chosen}
                  onClick={() => pick(type.id)}
                >
                  {type.name}
                </button>
              ))}
              {naming === null ? (
                <button
                  type="button"
                  className="add"
                  aria-label="新类型"
                  onClick={() => setNaming("")}
                >
                  ＋
                </button>
              ) : (
                <input
                  autoFocus
                  aria-label="新类型的名称"
                  placeholder="新类型"
                  value={naming}
                  disabled={addType.isPending}
                  onChange={(event) => setNaming(event.target.value)}
                  onKeyDown={nameKey}
                />
              )}
            </div>
            <div className="activity-times">
              <div>
                <Label htmlFor="activity-start">开始</Label>
                <Input
                  id="activity-start"
                  inputMode="numeric"
                  autoComplete="off"
                  placeholder="07:00"
                  value={startText}
                  aria-invalid={invalid?.field === "start"}
                  onChange={(event) => setStart(event.target.value)}
                />
              </div>
              <div>
                <Label htmlFor="activity-length">时长（分钟）</Label>
                <Input
                  id="activity-length"
                  inputMode="numeric"
                  autoComplete="off"
                  placeholder="30"
                  value={lengthText}
                  aria-invalid={invalid?.field === "length"}
                  onChange={(event) => {
                    setLength(event.target.value);
                    setLengthTyped(true);
                  }}
                />
              </div>
            </div>
            <div>
              <Label htmlFor="activity-note">备注</Label>
              <Input
                id="activity-note"
                autoComplete="off"
                value={note}
                onChange={(event) => setNote(event.target.value)}
              />
            </div>
          </fieldset>
          <p
            className={invalid ? "sleep-length invalid" : "sleep-length"}
            aria-live="polite"
          >
            {reading.kind === "activity"
              ? `${clock(reading.started_at)} – ${clock(reading.started_at + reading.minutes * 60_000)} · ${reading.minutes} 分钟`
              : invalid?.reason}
          </p>
          {addType.isError && <ErrorState error={addType.error} />}
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
                  删除
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
            <Button
              type="submit"
              disabled={busy || reading.kind === "invalid" || !chosen}
            >
              {save.isPending ? "正在保存…" : "保存"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
