import { tui, createTuiRenderer } from "./app"
import { ensureServer } from "./util/server"

const stopServer = await ensureServer()

process.stdout.write("\x1b]0;aaBot\x07")

const renderer = await createTuiRenderer()
const handle = tui({
  url: "http://localhost:3000",
  args: {},
  renderer,
})

await handle.done
stopServer()
