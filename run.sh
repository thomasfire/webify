#!/bin/bash

redis-server --port 6381 --daemonize yes --save "" --appendonly no --bind 127.0.0.1
redis-server --port 6380 --daemonize yes --bind 127.0.0.1
sh -c "./server 8080" &
RUST_LOG=webify=trace,actix_web=info,actix_server=info ./webify