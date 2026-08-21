import { useEffect, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { api } from "../api/client";

interface Props {
  url: string | null;
  alt: string;
  className?: string;
}

/**
 * Renders remote cover art through the Rust-side disk cache: the URL is
 * fetched at most once (subsequent renders of the same URL, even across app
 * restarts, hit the cached file) and served back as a local `asset://` src.
 */
export function CachedImage({ url, alt, className }: Props) {
  const [src, setSrc] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    let cancelled = false;
    setSrc(null);
    setFailed(false);
    if (!url) {
      setFailed(true);
      return;
    }
    api
      .fetchCoverImage(url)
      .then((localPath) => {
        if (!cancelled) setSrc(convertFileSrc(localPath));
      })
      .catch(() => {
        if (!cancelled) setFailed(true);
      });
    return () => {
      cancelled = true;
    };
  }, [url]);

  if (failed || !url) {
    return (
      <div className={className}>
        <span>No Cover</span>
      </div>
    );
  }

  if (!src) {
    return (
      <div className={className}>
        <span className="spinner" />
      </div>
    );
  }

  return (
    <div className={className}>
      <img src={src} alt={alt} loading="lazy" />
    </div>
  );
}
