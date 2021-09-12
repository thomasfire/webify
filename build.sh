#!/bin/bash

DIR="$(dirname "$0")"

if cargo "$@"; then
    [ -d "$DIR/target/debug" ] && cp -r "$DIR/static" "$DIR/target/debug/" && cp -r "$DIR/templates" "$DIR/target/debug/" && cp -r "run.sh" "$DIR/target/debug/"
    [ -d "$DIR/target/release" ] && cp -r "$DIR/static" "$DIR/target/release/" && cp -r "$DIR/templates" "$DIR/target/release/" && cp -r "run.sh" "$DIR/target/debug/"
fi

