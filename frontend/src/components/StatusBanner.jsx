const TONES = {
  amber: 'bg-amber-50 border-amber-200 text-amber-800',
  red: 'bg-red-50 border-red-200 text-red-800',
  green: 'bg-green-50 border-green-200 text-green-900',
  blue: 'bg-blue-50 border-blue-200 text-blue-900',
};

/// Inline notice block (config warnings, info callouts).
export function StatusBanner({ tone = 'amber', className = '', children }) {
  return (
    <div className={`p-4 border rounded-lg ${TONES[tone] || TONES.amber} ${className}`}>
      {children}
    </div>
  );
}
