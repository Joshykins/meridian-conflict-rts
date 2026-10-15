import { ValleyField } from "./valley-field";
import site from "../../content/site.json";

/**
 * The hero's moving picture: battle footage the game filmed itself
 * (`scripts/film.sh`), muted and looped over the live valley, which stays
 * underneath as its loading state. With reduced motion only the poster shows.
 */
export function HeroMedia() {
  const video = site.heroVideo as { sources: { src: string; type: string }[]; poster: string | null };
  return (
    <div className="absolute inset-0">
      <ValleyField />
      {video.sources.length > 0 && (
        <video
          className="absolute inset-0 h-full w-full object-cover motion-reduce:hidden"
          poster={video.poster ?? undefined}
          autoPlay
          muted
          loop
          playsInline
          preload="auto"
          aria-hidden
        >
          {video.sources.map((s) => (
            <source key={s.src} src={s.src} type={s.type} />
          ))}
        </video>
      )}
      {video.poster && (
        // eslint-disable-next-line @next/next/no-img-element -- a still for reduced motion, sized by the hero
        <img src={video.poster} alt="" className="absolute inset-0 hidden h-full w-full object-cover motion-reduce:block" />
      )}
    </div>
  );
}
