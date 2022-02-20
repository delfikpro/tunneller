FROM alpine
WORKDIR /app
ENTRYPOINT ./tunneller
EXPOSE 34064
COPY tunneller ./tunneller