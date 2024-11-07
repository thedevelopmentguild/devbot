FROM archlinux:latest
WORKDIR /bot

COPY target/release/devbot .
COPY start-bot.sh .
COPY redis.conf .
COPY .env .

RUN chmod +x devbot
RUN pacman -Sy redis --noconfirm

CMD ["./start-bot.sh"]
