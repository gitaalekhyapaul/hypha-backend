import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { LoginView } from "../components/login-view.tsx";
import { OperatorPortal } from "../components/operator-portal.tsx";

const css = readFileSync(new URL("../app/globals.css", import.meta.url), "utf8");

describe("Chamber Lantern operator branding", () => {
  it.each([
    [
      "public/brand/logos/logo-primary-light.svg",
      "e9d24dcdc1b43223641c45b4b712478f1cec618b3ceba65a34d63d6a8dd4dd95",
    ],
    [
      "public/brand/logos/logo-symbol-dark.svg",
      "3f4e3beef79d2b10be1d056cb7788fef34c381654e2f0788dcaf786a40a49291",
    ],
    ["app/icon.svg", "964ba9caad76b3941d030a744fd254f35db7718d005ebcad1ab6c0aa5876c02e"],
  ])("preserves the exact approved artwork in %s", (path, expected) => {
    const asset = readFileSync(new URL(`../${path}`, import.meta.url));
    expect(createHash("sha256").update(asset).digest("hex")).toBe(expected);
  });

  it("keeps the sign-in form and identifies the operator and synthetic scenario", () => {
    const markup = renderToStaticMarkup(
      <LoginView busy={false} error={null} onLogin={async () => {}} />,
    );
    expect(markup).toContain('src="/brand/logos/logo-primary-light.svg"');
    expect(markup).toContain('alt="Raziel"');
    expect(markup).toContain("Operator console");
    expect(markup).toContain("Synthetic workspace");
    expect(markup).toContain("Payment reconciliation");
    expect(markup).toContain('type="password"');
    expect(markup).toContain('autoComplete="current-password"');
    expect(markup).toContain('type="submit"');
    expect(markup).not.toMatch(/\bM[56]\b/);
  });

  it("retains disabled and error states in the branded login", () => {
    const markup = renderToStaticMarkup(
      <LoginView busy={true} error="Operator service unavailable" onLogin={async () => {}} />,
    );
    expect(markup.match(/disabled=""/g)).toHaveLength(2);
    expect(markup).toContain("Signing in...");
    expect(markup).toContain('role="alert"');
    expect(markup).toContain("Operator service unavailable");
  });

  it.each(["legacy", "versioned"] as const)(
    "uses the approved identity during %s session loading",
    (policyWorkflow) => {
      const markup = renderToStaticMarkup(<OperatorPortal policyWorkflow={policyWorkflow} />);
      expect(markup).toContain('src="/brand/logos/logo-primary-light.svg"');
      expect(markup).toContain("Opening operator console...");
      expect(markup).toContain('aria-live="polite"');
      expect(markup).not.toMatch(/\bM[56]\b/);
    },
  );

  it("serves every font locally with its supplied Open Font License", () => {
    const fontPaths = [...css.matchAll(/url\("(\/brand\/fonts\/[^"]+)"\)/g)].map(
      (match) => match[1],
    );
    expect(fontPaths).toEqual([
      "/brand/fonts/manrope.ttf",
      "/brand/fonts/publicsans.ttf",
      "/brand/fonts/ibmplexmono.ttf",
      "/brand/fonts/ibmplexmono-medium.ttf",
    ]);
    for (const path of fontPaths) {
      expect(readFileSync(new URL(`../public${path}`, import.meta.url)).byteLength).toBeGreaterThan(
        1000,
      );
    }
    for (const family of ["manrope", "publicsans", "ibmplexmono"]) {
      const license = readFileSync(
        new URL(`../public/brand/fonts/${family}-OFL.txt`, import.meta.url),
        "utf8",
      );
      expect(license).toContain("SIL OPEN FONT LICENSE");
    }
    expect(css).not.toMatch(/https?:\/\/|font-geist|\bInter\b/);
    expect(css).not.toMatch(/letter-spacing:\s*-[\d.]|font-size:[^;]*\b(?:vw|cqw)\b/);
  });

  it.each([
    ["brand-ink", "brand-paper", 4.5],
    ["brand-jade", "brand-paper", 4.5],
    ["brand-white", "brand-cobalt", 4.5],
    ["brand-mint", "brand-dark", 4.5],
    ["brand-mint", "brand-panel", 4.5],
    ["brand-ink", "brand-citrine", 4.5],
    ["text-muted", "brand-muted", 4.5],
    ["control-border", "brand-white", 3],
  ])("keeps %s on %s above %s:1 contrast", (foreground, background, minimum) => {
    const light = Math.max(luminance(token(foreground)), luminance(token(background)));
    const dark = Math.min(luminance(token(foreground)), luminance(token(background)));
    expect((light + 0.05) / (dark + 0.05)).toBeGreaterThanOrEqual(minimum);
  });
});

function token(name: string): string {
  const value = css.match(new RegExp(`--${name}: (#[0-9a-f]{6});`))?.[1];
  if (!value) throw new Error(`Missing brand color ${name}`);
  return value;
}

function luminance(hex: string): number {
  return (
    [
      [1, 0.2126],
      [3, 0.7152],
      [5, 0.0722],
    ] as const
  ).reduce((total, [start, weight]) => {
    const channel = Number.parseInt(hex.slice(start, start + 2), 16) / 255;
    const linear = channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4;
    return total + linear * weight;
  }, 0);
}
