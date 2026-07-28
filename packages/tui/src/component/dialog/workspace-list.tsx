import { DialogSelect, type SelectOption } from "../../ui/dialog-select"
import { useWorkspace } from "../../context/workspace"

interface WorkspaceListDialogProps {
  onSelect?: (dir: string) => void
}

export function WorkspaceListDialog(props: WorkspaceListDialogProps) {
  const ws = useWorkspace()

  const options = (): SelectOption<string>[] => {
    const dirs = [ws.directory(), ...ws.recent().filter((d) => d !== ws.directory())]
    return dirs.map((dir) => ({
      title: dir.replace(process.env.HOME || "", "~"),
      description: ws.branch() && dir === ws.directory() ? ws.branch()! : undefined,
      value: dir,
    }))
  }

  return (
    <DialogSelect
      title="Switch Workspace"
      options={options()}
      placeholder="Search or type a path..."
      emptyMessage="No workspaces"
      onSelect={(dir) => {
        ws.switchTo(dir)
        props.onSelect?.(dir)
      }}
    />
  )
}
