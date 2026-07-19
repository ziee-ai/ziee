# Docker-only musl build image for ziee. Lets the CI/build agent compile the
# static-musl release binary with ONLY Docker installed — no host rust / zig /
# cargo-zigbuild.
#
# messense/cargo-zigbuild bundles zig + cargo-zigbuild but its 0.20.0 tag ships
# rustc 1.85, while the ziee dependency graph requires >= 1.88 (darling/home
# 1.88, built 1.87, icu 1.86). So bump the Rust toolchain to 1.97.0 (the version
# the host build used); zig + cargo-zigbuild from the base image are reused.
#
#   docker build -f deploy/runtime/zigbuild.Dockerfile -t ziee-zigbuild:1.97 .
FROM messense/cargo-zigbuild:0.20.0
RUN rustup toolchain install 1.97.0 --profile minimal --target x86_64-unknown-linux-musl \
 && rustup default 1.97.0
