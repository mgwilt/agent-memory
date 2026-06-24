FROM rust:1.85-bookworm AS build
ENV RUSTUP_TOOLCHAIN=1.85.0
WORKDIR /app
COPY . .
RUN cargo build --release --locked -p nestor-api

FROM debian:bookworm-slim
RUN useradd --create-home --shell /usr/sbin/nologin nestor
COPY --from=build /app/target/release/nestor-api /usr/local/bin/nestor-api
ENV NESTOR_API_BIND_ADDR=0.0.0.0:8080
USER nestor
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/nestor-api"]
CMD ["serve"]
