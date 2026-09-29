"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { useEffect, useState } from "react";
import { Emblem, Wordmark } from "./emblem";
import site from "../../content/site.json";

const NAV = [
  { href: "/units", label: "Units" },
  { href: "/factions/arc", label: "Factions", match: "/factions" },
  { href: "/#war", label: "The war", match: "/#" },
];

export function SiteHeader() {
  const pathname = usePathname();
  const [scrolled, setScrolled] = useState(false);

  useEffect(() => {
    const on = () => setScrolled(window.scrollY > 8);
    on();
    window.addEventListener("scroll", on, { passive: true });
    return () => window.removeEventListener("scroll", on);
  }, []);

  return (
    <header
      style={{ viewTransitionName: "site-header" }}
      className={`fixed inset-x-0 top-0 z-50 h-[var(--header-h)] transition-[background-color,border-color,backdrop-filter] duration-300 ${
        scrolled ? "border-b border-white/10 bg-black/55 backdrop-blur-xl" : "border-b border-transparent"
      }`}
    >
      <div className="mx-auto flex h-full max-w-[1480px] items-center gap-4 px-4 sm:px-8">
        <Link
          href="/"
          transitionTypes={["nav-fade"]}
          className="group flex items-center gap-3"
          aria-label="Meridian Conflict home"
        >
          <Emblem className="size-8 transition-transform duration-300 group-hover:scale-105" />
          <Wordmark className="hidden text-[1.35rem] sm:inline" />
        </Link>

        <nav className="ml-auto flex items-center gap-1 text-[0.95rem]">
          {NAV.map((n) => {
            const active = n.match ? pathname.startsWith(n.match) : pathname.startsWith(n.href);
            return (
              <Link
                key={n.href}
                href={n.href}
                transitionTypes={["nav-fade"]}
                className={`relative px-3 py-2 font-medium transition-colors ${
                  active ? "text-text" : "text-dim hover:text-text"
                } ${n.label === "The war" ? "hidden md:block" : ""}`}
              >
                {n.label}
                <span
                  className={`absolute inset-x-3 -bottom-px h-[2px] origin-left bg-accent transition-transform duration-300 ease-out ${
                    active ? "scale-x-100" : "scale-x-0"
                  }`}
                />
              </Link>
            );
          })}
          <a
            href={site.discord}
            target="_blank"
            rel="noreferrer"
            className="ml-2 inline-flex items-center gap-2 border border-white/15 bg-white/[0.04] px-3.5 py-1.5 font-display font-semibold text-text transition hover:border-accent/70 hover:bg-accent/10"
          >
            <span className="size-1.5 animate-[blink_2s_ease-in-out_infinite] rounded-full bg-accent" />
            Discord
          </a>
        </nav>
      </div>
    </header>
  );
}
