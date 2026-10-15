import site from "../../content/site.json";

/** The launcher's install zip: the playtest channel's newest build. */
export function DownloadLink({ className = "", children }: { className?: string; children?: React.ReactNode }) {
  return (
    <a href={site.download.href} download className={className}>
      {children ?? (
        <>
          <svg viewBox="0 0 16 16" className="size-4" aria-hidden>
            <path d="M8 1v9M4 6.5L8 10.5L12 6.5M2 14h12" fill="none" stroke="currentColor" strokeWidth="1.8" />
          </svg>
          Download the launcher
        </>
      )}
    </a>
  );
}

/** "Open playtest", with a live dot: the game's stage, wherever it is named. */
export function PlaytestBadge({ className = "" }: { className?: string }) {
  return (
    <span className={`inline-flex items-center gap-2 ${className}`}>
      <span className="relative flex size-2">
        <span className="absolute inset-0 animate-ping rounded-full bg-accent opacity-60 motion-reduce:hidden" />
        <span className="relative size-2 rounded-full bg-accent" />
      </span>
      Open playtest
    </span>
  );
}
