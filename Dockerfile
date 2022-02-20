FROM rust AS build
COPY . /app
WORKDIR /app
RUN cargo build --release

FROM alpine
WORKDIR /app
COPY --from=build /app/target/release/tunneller ./tunneller