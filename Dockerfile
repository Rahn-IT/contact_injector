FROM rust AS builder
WORKDIR /app
COPY *.toml .
COPY Cargo.lock .
COPY ./.sqlx ./.sqlx
COPY ./contact_injector ./contact_injector
COPY ./contact_protocols ./contact_protocols
RUN cargo build --release

FROM debian:stable-slim AS runner
RUN mkdir -p /app/db
WORKDIR /app
COPY --from=builder /app/target/release/contact_injector /app/contact_injector
EXPOSE 4040
VOLUME /app/db
CMD ["/app/contact_injector"]
