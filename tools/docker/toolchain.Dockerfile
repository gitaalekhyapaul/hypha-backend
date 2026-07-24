# syntax=docker/dockerfile:1.7

ARG NODE_IMAGE
ARG GO_IMAGE
ARG BUF_IMAGE
ARG RUST_IMAGE

FROM ${NODE_IMAGE} AS node-toolchain
FROM ${GO_IMAGE} AS go-toolchain
FROM ${BUF_IMAGE} AS buf-toolchain
FROM ${RUST_IMAGE} AS toolchain

ARG PNPM_VERSION
ARG PROTOC_VERSION
ARG PROTOBUF_COMPILER_PACKAGE_VERSION
ARG LIBPROTOBUF_DEV_PACKAGE_VERSION
ARG SP1_VERSION
ARG SP1_CARGO_PROVE_AMD64_SHA256
ARG SP1_CARGO_PROVE_ARM64_SHA256
ARG TARGETARCH

COPY --from=node-toolchain /usr/local/ /usr/local/
COPY --from=go-toolchain /usr/local/go/ /usr/local/go/
COPY --from=buf-toolchain /usr/local/bin/buf /usr/local/bin/buf

ENV PATH="/root/.sp1/bin:/pnpm/bin:/usr/local/go/bin:${PATH}" \
    PNPM_HOME="/pnpm" \
    PNPM_STORE_DIR="/pnpm/store"

RUN apt-get update \
    && apt-get install --yes --no-install-recommends \
      "protobuf-compiler=${PROTOBUF_COMPILER_PACKAGE_VERSION}" \
      "libprotobuf-dev=${LIBPROTOBUF_DEV_PACKAGE_VERSION}" \
    && test "$(protoc --version)" = "libprotoc ${PROTOC_VERSION}" \
    && test -f /usr/include/google/protobuf/empty.proto \
    && rm -rf /var/lib/apt/lists/* \
    && npm install --global "pnpm@${PNPM_VERSION}" \
    && mkdir -p "${PNPM_STORE_DIR}" \
    && pnpm config set store-dir "${PNPM_STORE_DIR}" --global \
    && mkdir -p /root/.sp1/bin \
    && case "${TARGETARCH}" in \
      amd64) sp1_sha256="${SP1_CARGO_PROVE_AMD64_SHA256}" ;; \
      arm64) sp1_sha256="${SP1_CARGO_PROVE_ARM64_SHA256}" ;; \
      *) echo "unsupported SP1 architecture: ${TARGETARCH}" >&2; exit 1 ;; \
    esac \
    && curl --proto '=https' --tlsv1.2 --fail --silent --show-error --location \
      "https://github.com/succinctlabs/sp1/releases/download/v${SP1_VERSION}/cargo_prove_v${SP1_VERSION}_linux_${TARGETARCH}.tar.gz" \
      --output /tmp/cargo-prove.tar.gz \
    && echo "${sp1_sha256}  /tmp/cargo-prove.tar.gz" | sha256sum --check --strict - \
    && tar -xzf /tmp/cargo-prove.tar.gz -C /root/.sp1/bin \
    && rm /tmp/cargo-prove.tar.gz \
    && cargo prove install-toolchain \
    && cargo prove --version

WORKDIR /workspace

CMD ["bash"]
