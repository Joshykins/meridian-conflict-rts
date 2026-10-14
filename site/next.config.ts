import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  poweredByHeader: false,
  reactStrictMode: true,
  // A server with only what it needs (deploy/site.Dockerfile runs it on Railway).
  output: "standalone",
  // Every page is static: built once from content/*.json and served from the edge cache.
  async headers() {
    return [
      {
        source: "/:path*",
        headers: [{ key: "X-Content-Type-Options", value: "nosniff" }],
      },
      {
        // Unit meshes and portraits (scripts/models.mjs): each address carries its
        // content's hash, so a browser may keep one for good.
        source: "/generated/:path*",
        headers: [{ key: "Cache-Control", value: "public, max-age=31536000, immutable" }],
      },
    ];
  },
};

export default nextConfig;
