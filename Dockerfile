FROM rust:latest AS builder
COPY . .
RUN cargo build --release

FROM archlinux:latest
WORKDIR /bot
RUN pacman -Sy redis --noconfirm

COPY --from=builder ./target/release/devbot ./devbot
COPY redis.conf .
COPY start-bot.sh .
RUN chmod +x start-bot.sh

CMD ["./start-bot.sh"]
