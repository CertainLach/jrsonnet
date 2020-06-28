#!/usr/bin/env sh
export DOCKER_BUILDKIT=1
docker build -t jrsonnet -f build/Dockerfile build/
docker run --rm -it -v $PWD:/build -e CARGO_HOME=/build/cache jrsonnet:latest ash -c "cd /build&&$@"
