FROM alpine
WORKDIR /app
ENTRYPOINT ./tunneller
EXPOSE 34064
COPY target/release/tunneller ./tunneller