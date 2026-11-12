import type { NavItem } from "../nav";

export default function Planned({ item }: { item: NavItem }) {
  return (
    <div className="card">
      <span className={`tag ${item.status}`}>{item.status === "planned" ? "Planned: not available yet" : "Partly available"}</span>
      <p>{item.summary}</p>
      {item.needs && (
        <>
          <h3>Will need</h3>
          <ul>{item.needs.map((n) => <li key={n}>{n}</li>)}</ul>
        </>
      )}
      {item.next && <p className="muted">Next step: {item.next}</p>}
    </div>
  );
}
