import type { StatusItem } from "../lib/console-status";

interface StatusTableProps {
  items: readonly StatusItem[];
}

export function StatusTable({ items }: StatusTableProps) {
  return (
    <dl className="status-list">
      {items.map((item) => (
        <div className="status-row" key={item.id}>
          <dt>{item.label}</dt>
          <dd>
            <span aria-hidden="true" className="status-dot" data-state={item.state} />
            {item.detail}
          </dd>
        </div>
      ))}
    </dl>
  );
}
