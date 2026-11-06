import { AppWindow } from "lucide-react";
import { useState } from "react";
import { useProcessIcon, useRowIcon } from "./processIcons";

/** The app's own icon, or a neutral window glyph while unknown or missing. */
export function AppIcon({
  src,
  size = 16,
  className = "",
}: {
  src: string | null;
  size?: number;
  className?: string;
}) {
  const [broken, setBroken] = useState<string | null>(null);
  if (src && broken !== src)
    return (
      <img
        className={`app-icon ${className}`}
        src={src}
        width={size}
        height={size}
        alt=""
        aria-hidden="true"
        draggable={false}
        onError={() => setBroken(src)}
      />
    );
  return (
    <AppWindow
      size={size}
      aria-hidden="true"
      className={`app-icon app-icon-fallback ${className}`}
    />
  );
}

export function RowIcon(props: {
  revision: number | undefined;
  reference: string | undefined;
  processKey: string;
}) {
  return (
    <AppIcon
      src={useRowIcon(props.revision, props.reference, props.processKey)}
    />
  );
}

export function ProcessIcon({
  processKey,
  className,
}: {
  processKey: string;
  className?: string;
}) {
  return <AppIcon src={useProcessIcon(processKey)} className={className} />;
}
