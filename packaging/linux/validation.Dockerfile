FROM ubuntu:22.04
ENV DEBIAN_FRONTEND=noninteractive
# Deliberately do not install GTK or WebKitGTK. Display/graphics and glibc are
# the declared host boundary; the suite must supply its entire UI runtime.
RUN apt-get update && apt-get install -y --no-install-recommends \
    python3 xvfb xauth dbus-x11 libgl1 libegl1 libgbm1 libasound2 ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --uid 1000 smoke
USER smoke
WORKDIR /home/smoke
