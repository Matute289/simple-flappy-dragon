FROM golang:1.22-alpine AS builder
WORKDIR /app
COPY go.mod main.go ./
RUN CGO_ENABLED=0 GOOS=linux go build -o scores-api .

FROM alpine:3.19
RUN mkdir -p /data
COPY --from=builder /app/scores-api /scores-api
EXPOSE 8080
ENTRYPOINT ["/scores-api"]
