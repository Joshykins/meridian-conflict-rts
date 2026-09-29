import Link from "next/link";

export default function NotFound() {
  return (
    <div className="mx-auto flex min-h-[80vh] max-w-[1480px] flex-col items-start justify-center px-4 pt-[var(--header-h)] sm:px-8">
      <p className="num font-display text-sm font-semibold text-accent">404 · No contact</p>
      <h1 className="mt-3 font-display text-6xl font-light sm:text-8xl">Nothing on file here</h1>
      <p className="mt-4 max-w-lg text-lg text-dim">
        The page you asked for isn&apos;t in the record. It may have moved, or it was never built.
      </p>
      <div className="mt-8 flex gap-3">
        <Link
          href="/"
          className="bg-accent px-5 py-3 font-display text-lg font-semibold text-black transition hover:brightness-110"
        >
          Back to base
        </Link>
        <Link
          href="/units"
          className="border border-white/15 px-5 py-3 font-display text-lg font-semibold transition hover:border-white/30"
        >
          Unit directory
        </Link>
      </div>
    </div>
  );
}
