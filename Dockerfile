FROM rust:1.89.0-slim-bookworm

WORKDIR /app
RUN env

RUN ln -sf /usr/share/zoneinfo/Asia/Shanghai /etc/localtime
RUN echo "Asia/Shanghai" > /etc/timezone

RUN rm /etc/apt/sources.list* -rf
COPY sources.list /etc/apt/sources.list
RUN apt-get clean && apt-get update && apt-get install -y pkg-config libssl-dev docker-compose docker.io 

ENV CARGO_HOME=/usr/local/cargo
RUN mkdir -p $CARGO_HOME
COPY rsproxy.conf.toml $CARGO_HOME/config.toml

COPY . /app

RUN cargo build --release


ENTRYPOINT [ "/app/target/release/floatctf" ]