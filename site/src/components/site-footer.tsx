import Link from "next/link";
import { Emblem, Wordmark } from "./emblem";
import site from "../../content/site.json";

export function SiteFooter() {
  return (
    <footer className="relative border-t border-white/10 bg-black">
      <div className="mx-auto grid max-w-[1480px] gap-10 px-4 py-14 sm:px-8 md:grid-cols-[1.4fr_1fr_1fr]">
        <div>
          <div className="flex items-center gap-3">
            <Emblem className="size-9" />
            <Wordmark className="text-2xl" />
          </div>
          <p className="mt-4 max-w-sm text-dim">
            A real-time strategy game about building bases and commanding huge armies. Currently in development.
          </p>
        </div>
        <FooterCol
          title="Explore"
          links={[
            { href: "/units", label: "Unit directory" },
            { href: "/factions/arc", label: "Asterian Reach Command" },
            { href: "/factions/regency", label: "The Regency" },
          ]}
        />
        <FooterCol title="Community" links={[{ href: site.discord, label: "Discord", external: true }]} />
      </div>
      <div className="mx-auto flex max-w-[1480px] flex-wrap items-center justify-between gap-2 border-t border-white/5 px-4 py-5 text-sm text-faint sm:px-8">
        <span>In development. Everything shown is work in progress.</span>
        <span className="num">© {new Date().getFullYear()} Meridian Conflict</span>
      </div>
    </footer>
  );
}

function FooterCol({ title, links }: { title: string; links: { href: string; label: string; external?: boolean }[] }) {
  return (
    <div>
      <h3 className="font-display text-sm font-semibold text-faint">{title}</h3>
      <ul className="mt-3 space-y-2">
        {links.map((l) => (
          <li key={l.href}>
            {l.external ? (
              <a href={l.href} target="_blank" rel="noreferrer" className="text-dim transition hover:text-accent">
                {l.label} ↗
              </a>
            ) : (
              <Link href={l.href} transitionTypes={["nav-fade"]} className="text-dim transition hover:text-accent">
                {l.label}
              </Link>
            )}
          </li>
        ))}
      </ul>
    </div>
  );
}
