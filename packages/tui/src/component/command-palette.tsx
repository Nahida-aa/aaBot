import { DialogSelect, type SelectOption } from "../ui/dialog-select"
import { useDialog } from "../ui/dialog"

export type PaletteCommand =
  | { action: "config" }
  | { action: "session-list" }
  | { action: "new-session" }
  | { action: "back-home" }
  | { action: "help" }

interface CommandPaletteProps {
  onCommand: (cmd: PaletteCommand) => void
}

const COMMANDS: SelectOption<PaletteCommand>[] = [
  { title: "Change Model / Provider", description: "Switch LLM model or provider", value: { action: "config" }, category: "Settings" },
  { title: "Open Session", description: "Browse and resume past sessions", value: { action: "session-list" }, category: "Session" },
  { title: "New Session", description: "Start a fresh conversation", value: { action: "new-session" }, category: "Session" },
  { title: "Back to Home", description: "Return to the home screen", value: { action: "back-home" }, category: "Navigation" },
  { title: "Help", description: "Show keybindings and usage tips", value: { action: "help" }, category: "Help" },
]

export function CommandPalette(props: CommandPaletteProps) {
  return (
    <DialogSelect
      title="Command Palette"
      options={COMMANDS}
      placeholder="Type a command..."
      onSelect={(cmd) => props.onCommand(cmd)}
    />
  )
}
