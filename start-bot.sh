#!/bin/sh

redis-server &
echo "Waiting for the server to start"
sleep 4
echo "Starting the bot"
./devbot
