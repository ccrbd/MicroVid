import QueueList from "../components/QueueList";
import JobDetail from "../components/JobDetail";

export default function QueueView() {
  return (
    <div className="grid h-full" style={{ gridTemplateColumns: "minmax(320px, 1.1fr) minmax(360px, 1fr)" }}>
      <div className="min-h-0 border-r" style={{ borderColor: "var(--mv-border)", background: "var(--mv-bg)" }}>
        <QueueList />
      </div>
      <div className="min-h-0" style={{ background: "var(--mv-panel)" }}>
        <JobDetail />
      </div>
    </div>
  );
}
