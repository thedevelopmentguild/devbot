FROM archlinux:latest
WORKDIR /bot

COPY target/release/devbot .
COPY start-bot.sh .
COPY redis.conf .
COPY .env .

RUN mkdir redis
RUN chmod +x devbot
RUN pacman -Sy redis --noconfirm

CMD ["./start-bot.sh"]
