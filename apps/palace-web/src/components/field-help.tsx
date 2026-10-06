import type { ReactNode } from "react";
import { CircleHelp } from "lucide-react";
import { Tooltip } from "radix-ui";

// Rules a field must follow wait behind a small mark beside its label instead of a line under
// it, so the form shows only what is filled in. Hover or keyboard focus reveals them.
export function FieldHelp({
  label,
  children,
}: {
  label: string;
  children: ReactNode;
}) {
  return (
    <Tooltip.Provider delayDuration={150}>
      <Tooltip.Root>
        <Tooltip.Trigger
          type="button"
          className="field-help"
          aria-label={`${label}说明`}
        >
          <CircleHelp aria-hidden="true" />
        </Tooltip.Trigger>
        <Tooltip.Portal>
          <Tooltip.Content
            className="help-tip"
            side="top"
            align="start"
            sideOffset={6}
          >
            {children}
          </Tooltip.Content>
        </Tooltip.Portal>
      </Tooltip.Root>
    </Tooltip.Provider>
  );
}
