# ====== Stage 1: Build environment ======
FROM rust:1.89.0-slim-bookworm AS chef

WORKDIR /app

# 设置时区
ENV TZ=Asia/Shanghai
RUN ln -sf /usr/share/zoneinfo/$TZ /etc/localtime \
    && echo $TZ > /etc/timezone

# 安装构建依赖
RUN rm -rf /var/lib/apt/lists/*
RUN rm -rf /etc/apt/sources.list.d/*
COPY sources.list /etc/apt/sources.list
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*

# 安装 cargo-chef
RUN cargo install cargo-chef

ENV CARGO_HOME=/usr/local/cargo
RUN mkdir -p $CARGO_HOME
COPY rsproxy.conf.toml $CARGO_HOME/config.toml

# 复制 Cargo 配置文件，缓存依赖
COPY Cargo.toml Cargo.lock ./fcmc ./ 
RUN cargo chef prepare --recipe-path recipe.json
RUN rm -rf src

# ====== Stage 2: Build dependencies ======
FROM chef AS builder
COPY --from=chef /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# 复制完整项目并构建应用
COPY . .
RUN cargo build --release

# ====== Stage 3: Runtime ======
FROM debian:bookworm-slim

WORKDIR /app

# 时区设置
ENV TZ=Asia/Shanghai
RUN ln -sf /usr/share/zoneinfo/$TZ /etc/localtime \
    && echo $TZ > /etc/timezone

RUN rm -rf /var/lib/apt/lists/*
RUN rm -rf /etc/apt/sources.list.d/*
COPY sources.list /etc/apt/sources.list
# 安装运行时依赖
RUN apt-get update \
    && apt-get install -y --no-install-recommends libssl-dev \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*

# 拷贝可执行文件
COPY --from=builder /app/target/release/floatctf /app/floatctf


ENTRYPOINT ["/app/floatctf"]
