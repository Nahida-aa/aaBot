import { createContext, useContext, createSignal, createMemo, type ParentProps } from "solid-js"
import * as fs from "fs"

interface Workspace {
  directory: string
  branch?: string
}

function detectGitBranch(dir: string): string | undefined {
  try {
    const head = fs.readFileSync(`${dir}/.git/HEAD`, "utf-8").trim()
    const m = head.match(/^ref: refs\/heads\/(.+)$/)
    return m?.[1]
  } catch {
    return undefined
  }
}

function createWorkspaceState(initialDir: string) {
  const [directory, setDirectory] = createSignal(initialDir)
  const [recent, setRecent] = createSignal<string[]>([])

  const branch = createMemo(() => detectGitBranch(directory()))

  const workspace = createMemo((): Workspace => ({ directory: directory(), branch: branch() }))

  function switchTo(dir: string) {
    setDirectory(dir)
    setRecent((prev) => {
      const next = prev.filter((d) => d !== dir)
      return [dir, ...next].slice(0, 10)
    })
  }

  return { workspace, directory, branch, recent, switchTo }
}

type WorkspaceCtx = ReturnType<typeof createWorkspaceState>

const ctx = createContext<WorkspaceCtx>()

export function WorkspaceProvider(props: ParentProps) {
  const value = createWorkspaceState(process.cwd())
  return <ctx.Provider value={value}>{props.children}</ctx.Provider>
}

export function useWorkspace() {
  const v = useContext(ctx)
  if (!v) throw new Error("useWorkspace must be used within WorkspaceProvider")
  return v
}
