import { createResource } from "solid-js";
import { DialogSelect, type SelectOption } from "../../ui/dialog-select";
import { listSessions } from "@aa/sdk";

interface SessionListDialogProps {
  onSelect: (sessionId: string) => void;
}

export function SessionListDialog(props: SessionListDialogProps) {
  const [sessions] = createResource(() => listSessions().then((r) => r.data));

  const sessionOptions = (): SelectOption<string>[] => {
    const s = sessions();
    if (!s) return [];
    return s.map((session) => ({
      title: session.session_id.slice(0, 8),
      description: `${session.model} · ${session.message_count} msgs`,
      category: new Date(session.updated_at).toLocaleDateString(),
      value: session.session_id,
    }));
  };

  return (
    <DialogSelect
      title="Open Session"
      options={sessionOptions()}
      placeholder="Search sessions..."
      emptyMessage={sessions.loading ? "Loading..." : "No saved sessions"}
      onSelect={(id) => props.onSelect(id)}
    />
  );
}
