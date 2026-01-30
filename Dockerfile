# Runtime stage - using Ubuntu to match build environment glibc version
FROM ubuntu:24.04

# Copy the pre-built binaries (copied to docker-bin/ by the workflow)
COPY docker-bin/rtk /usr/local/bin/rtk
COPY docker-bin/jrsonnet /usr/local/bin/jrsonnet

# Create a non-root user for security
RUN groupadd -r rtk && useradd -r -g rtk rtk
USER rtk

# Add labels for metadata
ARG VERSION
ARG BRANCH
ARG REVISION
LABEL org.opencontainers.image.title="rustanka" \
      org.opencontainers.image.description="Rust implementation of Tanka (rtk) with jrsonnet" \
      org.opencontainers.image.version="${VERSION}" \
      org.opencontainers.image.revision="${REVISION}" \
      org.opencontainers.image.source="https://github.com/grafana/rustanka"

ENTRYPOINT ["/usr/local/bin/rtk"]
CMD ["--help"]
