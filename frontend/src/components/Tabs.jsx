/// The setup-screen tab strip. Preserves the `.tab` / `.tab.active` styling.
export function Tabs({ tabs, active, onChange }) {
  return (
    <div className="flex gap-2 border-b border-gray-200 mb-6 flex-wrap">
      {tabs.map(tab => (
        <div
          key={tab.id}
          className={`tab ${tab.id === active ? 'active' : ''}`}
          onClick={() => onChange(tab.id)}
        >
          {tab.label}
        </div>
      ))}
    </div>
  );
}
