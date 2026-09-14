#!/bin/sh
set -eu

gdbus call --session \
    --dest org.freedesktop.portal.Desktop \
    --object-path /org/freedesktop/portal/desktop \
    --method org.freedesktop.host.portal.Registry.Register \
    io.github.NakuRei.pomolice '{}'
