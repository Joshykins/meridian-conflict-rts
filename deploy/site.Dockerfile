# The website (site/) in a container. Railway builds it from the `release`
# branch, so the site always shows the released game (docs/RELEASES.md). From
# the repository root:
#
#   docker build -f deploy/site.Dockerfile -t meridian-site .
#   docker run -d -p 3000:3000 meridian-site
#
# The whole repository goes in: the site's unit pages are made from the game's
# data and its model code (site/README.md), which needs the pinned Rust toolchain.
FROM node:22-bookworm AS build
ENV RUSTUP_HOME=/usr/local/rustup CARGO_HOME=/usr/local/cargo PATH=/usr/local/cargo/bin:$PATH
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
        | sh -s -- -y --profile minimal --default-toolchain none
WORKDIR /src
# The toolchain first, in a layer of its own: it changes far less than the code.
COPY rust-toolchain.toml ./
RUN rustup toolchain install
COPY . .
WORKDIR /src/site
RUN corepack enable && pnpm install --frozen-lockfile
RUN pnpm build

FROM node:22-bookworm-slim
WORKDIR /app
ENV NODE_ENV=production HOSTNAME=0.0.0.0 PORT=3000
COPY --from=build /src/site/.next/standalone ./
COPY --from=build /src/site/.next/static ./.next/static
COPY --from=build /src/site/public ./public
USER node
# Railway sets PORT; the server listens on it.
EXPOSE 3000
CMD ["node", "server.js"]
