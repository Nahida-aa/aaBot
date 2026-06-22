import { type Component, type JSX, createSignal, createMemo, For, Show, createEffect, onCleanup } from "solid-js"
import { useDialog } from "./dialog"
import { useTerminalDimensions } from "@opentui/solid"

export interface SelectOption<T = string> {
  title: string
  description?: string
  category?: string
  value: T
  disabled?: boolean
}

interface DialogSelectProps<T> {
  title: string
  options: SelectOption<T>[]
  onSelect: (value: T) => void
  onClose?: () => void
  placeholder?: string
  emptyMessage?: string
}

function fuzzyMatch(text: string, needle: string): boolean {
  if (!needle) return true
  const lower = needle.toLowerCase()
  const t = text.toLowerCase()
  let ti = 0
  for (let ni = 0; ni < lower.length; ni++) {
    ti = t.indexOf(lower[ni], ti)
    if (ti === -1) return false
    ti++
  }
  return true
}

export function DialogSelect<T = string>(props: DialogSelectProps<T>) {
  const dialog = useDialog()
  const dims = useTerminalDimensions()
  const [filter, setFilter] = createSignal("")
  const [selectedIdx, setSelectedIdx] = createSignal(0)

  const filtered = createMemo(() => {
    const needle = filter()
    const cats = new Map<string, SelectOption<T>[]>()
    for (const opt of props.options) {
      if (!needle || fuzzyMatch(opt.title + " " + (opt.category ?? ""), needle)) {
        const cat = opt.category ?? ""
        if (!cats.has(cat)) cats.set(cat, [])
        cats.get(cat)!.push(opt)
      }
    }
    return cats
  })

  const flatList = createMemo(() => {
    const result: { cat: string; opt: SelectOption<T> }[] = []
    for (const [cat, opts] of filtered()) {
      for (const opt of opts) {
        result.push({ cat, opt })
      }
    }
    return result
  })

  const visibleItems = createMemo(() => flatList().length)

  const handleSelect = (value: T) => {
    props.onSelect(value)
    dialog.pop()
  }

  return (
    <box flexDirection="column">
      {/* Header */}
      <box height={1} flexDirection="row">
        <text fg="#3b82f6">{props.title}</text>
        <box flexGrow={1} />
        <text fg="#666">Esc:close</text>
      </box>

      <box height={1} />

      {/* Search input */}
      <box height={1} flexDirection="row">
        <text fg="#888">&gt; </text>
        <textarea
          initialValue={filter()}
          placeholder={props.placeholder ?? "Search..."}
          placeholderColor="#555"
          minHeight={1}
          maxHeight={1}
          flexGrow={1}
          onContentChange={(v) => {
            if (typeof v === "string") {
              setFilter(v)
              setSelectedIdx(0)
            }
          }}
        />
      </box>

      <box height={1} />

      {/* Clear filter hint */}
      <Show when={filter()}>
        <text fg="#555">Esc to clear · Enter to select</text>
        <box height={1} />
      </Show>

      {/* Options list */}
      <scrollbox
        height={Math.min(visibleItems() * 2 + 4, dims().height - 12)}
        stickyScroll
      >
        <Show
          when={visibleItems() > 0}
          fallback={<text fg="#666">{props.emptyMessage ?? "No results"}</text>}
        >
          <For each={Array.from(filtered().entries())}>
            {([cat, opts]) => (
              <box flexDirection="column">
                <Show when={cat && !filter()}>
                  <text fg="#555">{cat}</text>
                </Show>
                <For each={opts}>
                  {(opt, i) => {
                    const globalIdx = flatList().findIndex(
                      (x) => x.opt === opt,
                    )
                    const isSelected = globalIdx === selectedIdx()
                    return (
                      <box
                        flexDirection="row"
                        backgroundColor={
                          isSelected ? "#3b82f644" : undefined
                        }
                        on:press={() => handleSelect(opt.value)}
                      >
                        <text
                          fg={
                            opt.disabled
                              ? "#555"
                              : isSelected
                                ? "#3b82f6"
                                : "#ccc"
                          }
                        >
                          {opt.title}
                        </text>
                        <Show when={opt.description}>
                          <text fg="#666"> — {opt.description}</text>
                        </Show>
                      </box>
                    )
                  }}
                </For>
              </box>
            )}
          </For>
        </Show>
      </scrollbox>

      {/* Event handlers for keyboard navigation */}
      <on:keypress
        fn={(e: { name?: string }) => {
          if (e.name === "up") {
            setSelectedIdx((i) => (i > 0 ? i - 1 : flatList().length - 1))
          } else if (e.name === "down") {
            setSelectedIdx((i) => (i < flatList().length - 1 ? i + 1 : 0))
          } else if (e.name === "return" || e.name === "enter") {
            const item = flatList()[selectedIdx()]
            if (item && !item.opt.disabled) handleSelect(item.opt.value)
          }
        }}
      />
    </box>
  )
}
