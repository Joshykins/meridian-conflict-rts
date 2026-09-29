import { ValleyField } from "./valley-field";
import site from "../../content/site.json";

/**
 * The hero's moving picture. The trailer is still to come: set
 * `heroVideo.src` in content/site.json and it plays here, muted and looped,
 * over the live valley (which stays underneath as its loading state).
 */
export function HeroMedia() {
  const video = site.heroVideo as { src: string | null; poster: string | null };
  return (
    <div className="absolute inset-0">
      <ValleyField />
      {video.src && (
        <video
          className="absolute inset-0 h-full w-full object-cover"
          src={video.src}
          poster={video.poster ?? undefined}
          autoPlay
          muted
          loop
          playsInline
          preload="metadata"
        />
      )}
    </div>
  );
}
