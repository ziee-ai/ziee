# Ziee server runtime image.
#
# The static musl binary is built OUTSIDE this image (it needs the pgvector
# build-DB + GITHUB_TOKEN for hub-seed + per-arch embedded Postgres), then
# COPYed in. buildx provides TARGETARCH (amd64/arm64); stage the matching
# binary at dist/ziee-${TARGETARCH} before `docker buildx build`.
#
# Defaults assume EXTERNAL Postgres + the code sandbox DISABLED. Embedded PG
# (manages its own data dir) and the bwrap sandbox (needs user namespaces /
# --privileged) both fight containerization — enable them only with the right
# volume + --privileged/--security-opt and host deps installed.
FROM alpine:3.20

RUN apk add --no-cache ca-certificates bubblewrap squashfuse fuse3 \
  && addgroup -S ziee && adduser -S -G ziee -H -h /var/lib/ziee ziee

ARG TARGETARCH
COPY dist/ziee-${TARGETARCH} /usr/local/bin/ziee
RUN chmod 0755 /usr/local/bin/ziee \
  && mkdir -p /var/lib/ziee/data && chown -R ziee:ziee /var/lib/ziee

USER ziee
VOLUME ["/var/lib/ziee"]
EXPOSE 9000

HEALTHCHECK --interval=30s --timeout=5s --start-period=30s \
  CMD wget -qO- http://127.0.0.1:9000/api/health || exit 1

# Mount a config at /etc/ziee/config.yaml (the image ships none — see the
# runbook). Run with `-v /path/config.yaml:/etc/ziee/config.yaml:ro`.
ENTRYPOINT ["/usr/local/bin/ziee"]
CMD ["--config-file", "/etc/ziee/config.yaml"]
