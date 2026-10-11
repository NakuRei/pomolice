FROM ubuntu:26.04
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
    dbus xdg-desktop-portal libglib2.0-bin
COPY artifacts/ubuntu26/*.deb /tmp/pomolice.deb
RUN dpkg-deb -x /tmp/pomolice.deb /
COPY docker/verify-portal-registration.sh /tmp/verify-registration.sh
RUN dbus-run-session -- sh /tmp/verify-registration.sh
