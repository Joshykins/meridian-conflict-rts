/**
 * Loading states: the page's own shape, shimmering, so nothing jumps when it
 * lands. Shown by NavProgress during a client navigation that has not arrived
 * within a moment (a route not yet prefetched), never on a direct load, which
 * gets the prerendered page at once.
 */
export function DirectorySkeleton() {
  return (
    <div className="mx-auto max-w-[1480px] px-4 pb-28 pt-[calc(var(--header-h)+2.5rem)] sm:px-8" aria-busy>
      <div className="skeleton h-4 w-32" />
      <div className="skeleton mt-4 h-16 w-full max-w-xl" />
      <div className="skeleton mt-5 h-5 w-full max-w-2xl" />
      <div className="skeleton mt-10 h-[106px] w-full" />
      <ul className="mt-8 grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5">
        {Array.from({ length: 10 }, (_, i) => (
          <li key={i} className="border border-white/[0.06]">
            <div className="skeleton aspect-[5/4]" style={{ animationDelay: `${i * 60}ms` }} />
            <div className="space-y-2 p-4">
              <div className="skeleton h-5 w-2/3" />
              <div className="skeleton h-3.5 w-1/2" />
            </div>
          </li>
        ))}
      </ul>
      <Loading />
    </div>
  );
}

export function UnitSkeleton() {
  return (
    <div className="mx-auto max-w-[1480px] px-4 pb-24 pt-[calc(var(--header-h)+1.5rem)] sm:px-8" aria-busy>
      <div className="skeleton h-5 w-36" />
      <div className="mt-6 grid gap-8 lg:grid-cols-[minmax(0,1.1fr)_minmax(0,1fr)] lg:gap-14">
        <div className="ticks relative aspect-square border border-white/[0.06]">
          <div className="absolute inset-[12%] rounded-full border border-dashed border-white/10" />
          <div className="absolute inset-x-0 top-1/2 h-px bg-gradient-to-r from-transparent via-accent/60 to-transparent [animation:scan_1.6s_ease-in-out_infinite]" />
        </div>
        <div className="space-y-4">
          <div className="skeleton h-4 w-56" />
          <div className="skeleton h-20 w-3/4" />
          <div className="skeleton h-7 w-1/2" />
          <div className="skeleton mt-6 h-20 w-full" />
          <div className="skeleton h-16 w-full" />
          {Array.from({ length: 5 }, (_, i) => (
            <div key={i} className="skeleton h-8 w-full" style={{ animationDelay: `${i * 80}ms` }} />
          ))}
        </div>
      </div>
      <Loading />
    </div>
  );
}

function Loading() {
  return (
    <p role="status" className="fixed bottom-6 left-1/2 -translate-x-1/2 font-display text-sm font-semibold text-dim">
      <span className="mr-2 inline-block size-1.5 animate-[blink_0.8s_ease-in-out_infinite] bg-accent" />
      Loading
    </p>
  );
}
