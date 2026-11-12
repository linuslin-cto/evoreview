# ---- Frontend build ----
FROM node:22-bookworm-slim AS web-build
WORKDIR /web
COPY web/package.json web/package-lock.json ./
RUN npm config set registry https://registry.npmmirror.com \
    && npm config set fetch-retries 5 \
    && npm config set fetch-retry-mintimeout 20000 \
    && npm config set fetch-retry-maxtimeout 120000 \
    && npm config set fetch-timeout 600000 \
    && npm ci
COPY web/ ./
ENV NEXT_TELEMETRY_DISABLED=1
ENV API_PROXY_TARGET=http://127.0.0.1:5001
RUN npm run build

# ---- Runtime ----
FROM python:3.10-slim

WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
    supervisor \
    git \
    ca-certificates \
    ripgrep \
    tree \
    file \
    curl \
    && curl -fsSL https://deb.nodesource.com/setup_22.x | bash - \
    && apt-get install -y --no-install-recommends nodejs \
    && rm -rf /var/lib/apt/lists/*

COPY requirements.txt .
RUN pip install --no-cache-dir -i https://mirrors.aliyun.com/pypi/simple/ -r requirements.txt

RUN mkdir -p log data conf
COPY biz ./biz
COPY assets ./assets
COPY api.py ./api.py
COPY conf/prompt_templates.yml ./conf/prompt_templates.yml
COPY conf/scr ./conf/scr
COPY conf/supervisord.conf /etc/supervisor/conf.d/supervisord.conf

COPY --from=web-build /web /app/web
ENV API_PROXY_TARGET=http://127.0.0.1:5001
ENV NEXT_TELEMETRY_DISABLED=1

EXPOSE 5001 5002

CMD ["/usr/bin/supervisord", "-c", "/etc/supervisor/conf.d/supervisord.conf"]
