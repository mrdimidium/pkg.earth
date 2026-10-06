# syntax=docker/dockerfile:1.7@sha256:a57df69d0ea827fb7266491f2813635de6f17269be881f696fbfdf2d83dda33e
# SPDX-FileCopyrightText: 2026 Nikolay Govorov
# SPDX-License-Identifier: MPL-2.0

ARG ALPINE_VERSION=3.23.5
ARG ALPINE_DIGEST=sha256:fd791d74b68913cbb027c6546007b3f0d3bc45125f797758156952bc2d6daf40
ARG DISTROLESS_DIGEST=sha256:a77defd6fedbb3392b175ba8ea3d1c22be963c1597c248c3ba987ddd80bfb512

FROM --platform=$BUILDPLATFORM docker.io/library/alpine:${ALPINE_VERSION}@${ALPINE_DIGEST} AS rootfs
RUN install -d -m 0750 /rootfs/var/lib/pkg-earth

FROM gcr.io/distroless/cc-debian13:nonroot@${DISTROLESS_DIGEST}

ARG TARGETARCH
ARG VERSION=dev
ARG REVISION=unknown

LABEL org.opencontainers.image.title="pkg.earth" \
      org.opencontainers.image.description="Tiny packages caching proxy" \
      org.opencontainers.image.source="https://gilti.dev/pkg.earth" \
      org.opencontainers.image.version="$VERSION" \
      org.opencontainers.image.revision="$REVISION" \
      org.opencontainers.image.licenses="MPL-2.0"

COPY --chown=root:root --chmod=0755 .container/binary-${TARGETARCH}/pkg-earth /usr/local/bin/pkg-earth
COPY --from=rootfs --chown=10000:10000 /rootfs/var/lib/pkg-earth /var/lib/pkg-earth
COPY --chown=root:root deploy/pkg-earth.toml /etc/pkg-earth.toml
COPY --chown=root:root LICENSE README.md /usr/share/doc/pkg-earth/

USER 10000:10000
EXPOSE 2025
VOLUME ["/var/lib/pkg-earth"]

ENTRYPOINT ["/usr/local/bin/pkg-earth"]
CMD ["--config=/etc/pkg-earth.toml"]
