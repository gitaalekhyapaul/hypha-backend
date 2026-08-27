import type { NextConfig } from "next";

const configuredDevOrigins = (process.env.RAZIEL_CONSOLE_ALLOWED_DEV_ORIGINS ?? "")
  .split(",")
  .map((origin) => origin.trim())
  .filter((origin) => origin.length > 0);

const nextConfig: NextConfig = {
  allowedDevOrigins: ["127.0.0.1", "localhost", ...configuredDevOrigins],
  devIndicators: false,
  poweredByHeader: false,
};

export default nextConfig;
