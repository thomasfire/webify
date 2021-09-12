#!/bin/bash

redis-server --port 6381 --daemonize yes --save "" --appendonly no
redis-server --port 6380 --daemonize yes
RUST_LOG=webify=trace,actix_web=info,actix_server=info ./webify