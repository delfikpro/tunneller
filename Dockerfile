FROM alpine
EXPOSE 34064
COPY tunneller /app/tunneller
WORKDIR /app/
ENTRYPOINT /app/tunneller