FROM rust:1-bookworm AS builder
WORKDIR /build
COPY backend/ backend/
COPY frontend/ frontend/
WORKDIR /build/backend
RUN cargo build --release -p news-server

FROM node:20-alpine AS minifier
WORKDIR /frontend
COPY frontend/ .
RUN npm init -y && npm install esbuild \
 && for f in js/*.js; do npx esbuild "$f" --minify --outfile="$f" --allow-overwrite; done \
 && for f in css/*.css; do npx esbuild "$f" --minify --outfile="$f" --allow-overwrite; done \
 && npx esbuild sw.js --minify --outfile=sw.js --allow-overwrite \
 && rm -rf node_modules package.json package-lock.json

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /build/backend/target/release/news-server /app/news-server
COPY --from=minifier /frontend/ /app/public/
EXPOSE 8080
ENV DATABASE_PATH=/data/news.db \
    STATIC_DIR=/app/public \
    RUST_LOG=info
CMD ["/app/news-server"]
