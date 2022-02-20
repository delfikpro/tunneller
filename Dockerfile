FROM alpine
WORKDIR /app
EXPOSE 34064
COPY tunneller /app/tunneller
ENTRYPOINT ./tunneller