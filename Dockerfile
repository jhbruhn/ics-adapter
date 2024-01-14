FROM --platform=$BUILDPLATFORM rust:1.75 as cross
ARG TARGETARCH
COPY docker/platform.sh .
RUN ./platform.sh # should write /.platform and /.compiler
RUN rustup target add $(cat /.platform)
RUN apt update && apt-get install -y unzip $(cat /.compiler)

WORKDIR ./ics-adapter
ADD . ./
RUN cargo build --release --target $(cat /.platform)
RUN cp ./target/$(cat /.platform)/release/ics-adapter /ics-adapter.bin # Get rid of this when build --out is stable


FROM debian:bookworm-slim
ARG APP=/usr/src/app

RUN apt-get update \
    && apt-get install -y ca-certificates tzdata \
    && rm -rf /var/lib/apt/lists/*

EXPOSE 3000

ENV TZ=Etc/UTC \
    APP_USER=appuser

RUN groupadd $APP_USER \
    && useradd -g $APP_USER $APP_USER \
    && mkdir -p ${APP}

COPY --from=cross /ics-adapter.bin ${APP}/ics-adapter

RUN chown -R $APP_USER:$APP_USER ${APP}

USER $APP_USER
WORKDIR ${APP}

CMD ["./ics-adapter"]
