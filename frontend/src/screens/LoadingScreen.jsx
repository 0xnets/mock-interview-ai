export function LoadingScreen({
  title = 'Preparing your interview...',
  subtitle = 'Analyzing the JD and resume to craft personalized questions',
}) {
  return (
    <div className="card p-8 text-center">
      <div className="text-6xl mb-4">🤖</div>
      <h2 className="text-2xl font-bold mb-2">{title}</h2>
      <p className="text-gray-600">{subtitle}</p>
      <div className="mt-6 flex justify-center">
        {[0, 1, 2, 3, 4].map(i => <div key={i} className="wave-bar" />)}
      </div>
    </div>
  );
}
