FROM rust:1.85-bookworm AS build
ENV RUSTUP_TOOLCHAIN=1.85.0
WORKDIR /app
COPY . .
RUN cargo build --release --locked -p actr-api

FROM debian:bookworm-slim
RUN useradd --create-home --shell /usr/sbin/nologin actr
COPY --from=build /app/target/release/actr-api /usr/local/bin/actr-api
ENV ACTR_API_BIND_ADDR=0.0.0.0:8080
USER actr
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/actr-api"]
CMD ["serve"]
