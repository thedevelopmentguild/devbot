FROM archlinux:latest
WORKDIR /bot

COPY target/debug/devbot .
COPY start-bot.sh .
COPY .env .

RUN chmod +x devbot
RUN pacman -Sy redis --noconfirm

CMD ["./start-bot.sh"]
