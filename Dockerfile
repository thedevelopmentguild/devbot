FROM rust:latest AS builder
COPY . .
RUN cargo build --release

FROM redis:latest
WORKDIR /bot
RUN apt-get update && apt-get install -y openssl

COPY --from=builder ./target/release/devbot ./devbot
COPY redis.conf .

COPY start-bot.sh .
RUN chmod +x start-bot.sh

CMD ["./start-bot.sh"]
