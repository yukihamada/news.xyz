# news.xyz

The $56,000 domain running the fastest AI news aggregator. 146+ feeds, AI summaries, voice news, 8 themes. Rust-powered. Ad-free.

## Development

```bash
# Build backend
cd backend
cargo build --release -p news-server

# Run locally
DATABASE_PATH=./news.db STATIC_DIR=../frontend cargo run -p news-server
```

## Deployment

Deployed to [Fly.io](https://fly.io) via GitHub Actions.

```bash
flyctl deploy
```

## Architecture

- **Backend**: Rust (axum) + SQLite
- **Frontend**: Vanilla JS PWA
- **Deploy**: Fly.io (news-xyz)
