import { locale, type Locale } from "./i18n";

/** Static backend duration, independent of polling and native work counters. */
export function scanDuration(
  milliseconds: number,
  language: Locale = locale,
): string {
  const unit =
    milliseconds < 1000
      ? "millisecond"
      : milliseconds < 60000
        ? "second"
        : "minute";
  const divisor = unit === "millisecond" ? 1 : unit === "second" ? 1000 : 60000;
  return new Intl.NumberFormat(language, {
    style: "unit",
    unit,
    unitDisplay: "short",
    maximumFractionDigits: unit === "millisecond" ? 0 : 1,
  }).format(milliseconds / divisor);
}
