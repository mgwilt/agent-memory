FROM rust:1-bookworm AS build
WORKDIR /app
COPY . .
RUN cargo build --release -p actr-api

FROM debian:bookworm-slim
RUN useradd --create-home --shell /usr/sbin/nologin actr
COPY --from=build /app/target/release/actr-api /usr/local/bin/actr-api
USER actr
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/actr-api"]
