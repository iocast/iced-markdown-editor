#!/usr/bin/env sh
# Builds the web demo into static/demo for `zola serve`.
set -e
cd "$(dirname "$0")/../examples/web"
trunk build --release --public-url /demo/ --dist ../../website/static/demo
